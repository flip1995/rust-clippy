//! Windows SEH
//!
//! On Windows (currently only on MSVC), the default exception handling
//! mechanism is Structured Exception Handling (SEH). This is quite different
//! than Dwarf-based exception handling (e.g., what other unix platforms use) in
//! terms of compiler internals, so LLVM is required to have a good deal of
//! extra support for SEH.
//!
//! In a nutshell, what happens here is:
//!
//! 1. The `panic` function calls the standard Windows function
//!    `_CxxThrowException` to throw a C++-like exception, triggering the
//!    unwinding process.
//! 2. All landing pads generated by the compiler use the personality function
//!    `__CxxFrameHandler3`, a function in the CRT, and the unwinding code in
//!    Windows will use this personality function to execute all cleanup code on
//!    the stack.
//! 3. All compiler-generated calls to `invoke` have a landing pad set as a
//!    `cleanuppad` LLVM instruction, which indicates the start of the cleanup
//!    routine. The personality (in step 2, defined in the CRT) is responsible
//!    for running the cleanup routines.
//! 4. Eventually the "catch" code in the `try` intrinsic (generated by the
//!    compiler) is executed and indicates that control should come back to
//!    Rust. This is done via a `catchswitch` plus a `catchpad` instruction in
//!    LLVM IR terms, finally returning normal control to the program with a
//!    `catchret` instruction.
//!
//! Some specific differences from the gcc-based exception handling are:
//!
//! * Rust has no custom personality function, it is instead *always*
//!   `__CxxFrameHandler3`. Additionally, no extra filtering is performed, so we
//!   end up catching any C++ exceptions that happen to look like the kind we're
//!   throwing. Note that throwing an exception into Rust is undefined behavior
//!   anyway, so this should be fine.
//! * We've got some data to transmit across the unwinding boundary,
//!   specifically a `Box<dyn Any + Send>`. Like with Dwarf exceptions
//!   these two pointers are stored as a payload in the exception itself. On
//!   MSVC, however, there's no need for an extra heap allocation because the
//!   call stack is preserved while filter functions are being executed. This
//!   means that the pointers are passed directly to `_CxxThrowException` which
//!   are then recovered in the filter function to be written to the stack frame
//!   of the `try` intrinsic.
//!
//! [win64]: http://msdn.microsoft.com/en-us/library/1eyas8tf.aspx
//! [llvm]: http://llvm.org/docs/ExceptionHandling.html#background-on-windows-exceptions

#![allow(nonstandard_style)]
#![allow(private_no_mangle_fns)]

use alloc::boxed::Box;
use core::any::Any;
use core::mem;
use core::raw;
use libc::{c_int, c_uint, c_void};

// First up, a whole bunch of type definitions. There's a few platform-specific
// oddities here, and a lot that's just blatantly copied from LLVM. The purpose
// of all this is to implement the `panic` function below through a call to
// `_CxxThrowException`.
//
// This function takes two arguments. The first is a pointer to the data we're
// passing in, which in this case is our trait object. Pretty easy to find! The
// next, however, is more complicated. This is a pointer to a `_ThrowInfo`
// structure, and it generally is just intended to just describe the exception
// being thrown.
//
// Currently the definition of this type [1] is a little hairy, and the main
// oddity (and difference from the online article) is that on 32-bit the
// pointers are pointers but on 64-bit the pointers are expressed as 32-bit
// offsets from the `__ImageBase` symbol. The `ptr_t` and `ptr!` macro in the
// modules below are used to express this.
//
// The maze of type definitions also closely follows what LLVM emits for this
// sort of operation. For example, if you compile this C++ code on MSVC and emit
// the LLVM IR:
//
//      #include <stdint.h>
//
//      struct rust_panic {
//          uint64_t x[2];
//      }
//
//      void foo() {
//          rust_panic a = {0, 1};
//          throw a;
//      }
//
// That's essentially what we're trying to emulate. Most of the constant values
// below were just copied from LLVM,
//
// In any case, these structures are all constructed in a similar manner, and
// it's just somewhat verbose for us.
//
// [1]: http://www.geoffchappell.com/studies/msvc/language/predefined/

#[cfg(target_arch = "x86")]
#[macro_use]
mod imp {
    pub type ptr_t = *mut u8;

    macro_rules! ptr {
        (0) => (core::ptr::null_mut());
        ($e:expr) => ($e as *mut u8);
    }
}

#[cfg(any(target_arch = "x86_64", target_arch = "arm"))]
#[macro_use]
mod imp {
    pub type ptr_t = u32;

    extern "C" {
        pub static __ImageBase: u8;
    }

    macro_rules! ptr {
        (0) => (0);
        ($e:expr) => {
            (($e as usize) - (&imp::__ImageBase as *const _ as usize)) as u32
        }
    }
}

#[repr(C)]
pub struct _ThrowInfo {
    pub attributes: c_uint,
    pub pnfnUnwind: imp::ptr_t,
    pub pForwardCompat: imp::ptr_t,
    pub pCatchableTypeArray: imp::ptr_t,
}

#[repr(C)]
pub struct _CatchableTypeArray {
    pub nCatchableTypes: c_int,
    pub arrayOfCatchableTypes: [imp::ptr_t; 1],
}

#[repr(C)]
pub struct _CatchableType {
    pub properties: c_uint,
    pub pType: imp::ptr_t,
    pub thisDisplacement: _PMD,
    pub sizeOrOffset: c_int,
    pub copy_function: imp::ptr_t,
}

#[repr(C)]
pub struct _PMD {
    pub mdisp: c_int,
    pub pdisp: c_int,
    pub vdisp: c_int,
}

#[repr(C)]
pub struct _TypeDescriptor {
    pub pVFTable: *const u8,
    pub spare: *mut u8,
    pub name: [u8; 11],
}

// Note that we intentionally ignore name mangling rules here: we don't want C++
// to be able to catch Rust panics by simply declaring a `struct rust_panic`.
const TYPE_NAME: [u8; 11] = *b"rust_panic\0";

static mut THROW_INFO: _ThrowInfo = _ThrowInfo {
    attributes: 0,
    pnfnUnwind: ptr!(0),
    pForwardCompat: ptr!(0),
    pCatchableTypeArray: ptr!(0),
};

static mut CATCHABLE_TYPE_ARRAY: _CatchableTypeArray = _CatchableTypeArray {
    nCatchableTypes: 1,
    arrayOfCatchableTypes: [ptr!(0)],
};

static mut CATCHABLE_TYPE: _CatchableType = _CatchableType {
    properties: 0,
    pType: ptr!(0),
    thisDisplacement: _PMD {
        mdisp: 0,
        pdisp: -1,
        vdisp: 0,
    },
    sizeOrOffset: mem::size_of::<[u64; 2]>() as c_int,
    copy_function: ptr!(0),
};

extern "C" {
    // The leading `\x01` byte here is actually a magical signal to LLVM to
    // *not* apply any other mangling like prefixing with a `_` character.
    //
    // This symbol is the vtable used by C++'s `std::type_info`. Objects of type
    // `std::type_info`, type descriptors, have a pointer to this table. Type
    // descriptors are referenced by the C++ EH structures defined above and
    // that we construct below.
    #[link_name = "\x01??_7type_info@@6B@"]
    static TYPE_INFO_VTABLE: *const u8;
}

// We use #[lang = "eh_catch_typeinfo"] here as this is the type descriptor which
// we'll use in LLVM's `catchpad` instruction which ends up also being passed as
// an argument to the C++ personality function.
//
// Again, I'm not entirely sure what this is describing, it just seems to work.
#[cfg_attr(not(test), lang = "eh_catch_typeinfo")]
static mut TYPE_DESCRIPTOR: _TypeDescriptor = _TypeDescriptor {
    pVFTable: unsafe { &TYPE_INFO_VTABLE } as *const _ as *const _,
    spare: core::ptr::null_mut(),
    name: TYPE_NAME,
};

pub unsafe fn panic(data: Box<dyn Any + Send>) -> u32 {
    use core::intrinsics::atomic_store;

    // _CxxThrowException executes entirely on this stack frame, so there's no
    // need to otherwise transfer `data` to the heap. We just pass a stack
    // pointer to this function.
    //
    // The first argument is the payload being thrown (our two pointers), and
    // the second argument is the type information object describing the
    // exception (constructed above).
    let ptrs = mem::transmute::<_, raw::TraitObject>(data);
    let mut ptrs = [ptrs.data as u64, ptrs.vtable as u64];
    let ptrs_ptr = ptrs.as_mut_ptr();
    let throw_ptr = ptrs_ptr as *mut _;

    // This... may seems surprising, and justifiably so. On 32-bit MSVC the
    // pointers between these structure are just that, pointers. On 64-bit MSVC,
    // however, the pointers between structures are rather expressed as 32-bit
    // offsets from `__ImageBase`.
    //
    // Consequently, on 32-bit MSVC we can declare all these pointers in the
    // `static`s above. On 64-bit MSVC, we would have to express subtraction of
    // pointers in statics, which Rust does not currently allow, so we can't
    // actually do that.
    //
    // The next best thing, then is to fill in these structures at runtime
    // (panicking is already the "slow path" anyway). So here we reinterpret all
    // of these pointer fields as 32-bit integers and then store the
    // relevant value into it (atomically, as concurrent panics may be
    // happening). Technically the runtime will probably do a nonatomic read of
    // these fields, but in theory they never read the *wrong* value so it
    // shouldn't be too bad...
    //
    // In any case, we basically need to do something like this until we can
    // express more operations in statics (and we may never be able to).
    atomic_store(&mut THROW_INFO.pCatchableTypeArray as *mut _ as *mut u32,
                 ptr!(&CATCHABLE_TYPE_ARRAY as *const _) as u32);
    atomic_store(&mut CATCHABLE_TYPE_ARRAY.arrayOfCatchableTypes[0] as *mut _ as *mut u32,
                 ptr!(&CATCHABLE_TYPE as *const _) as u32);
    atomic_store(&mut CATCHABLE_TYPE.pType as *mut _ as *mut u32,
                 ptr!(&TYPE_DESCRIPTOR as *const _) as u32);

    extern "system" {
        #[unwind(allowed)]
        pub fn _CxxThrowException(pExceptionObject: *mut c_void, pThrowInfo: *mut u8) -> !;
    }

    _CxxThrowException(throw_ptr,
                       &mut THROW_INFO as *mut _ as *mut _);
}

pub fn payload() -> [u64; 2] {
    [0; 2]
}

pub unsafe fn cleanup(payload: [u64; 2]) -> Box<dyn Any + Send> {
    mem::transmute(raw::TraitObject {
        data: payload[0] as *mut _,
        vtable: payload[1] as *mut _,
    })
}

// This is required by the compiler to exist (e.g., it's a lang item), but
// it's never actually called by the compiler because __C_specific_handler
// or _except_handler3 is the personality function that is always used.
// Hence this is just an aborting stub.
#[lang = "eh_personality"]
#[cfg(not(test))]
fn rust_eh_personality() {
    unsafe { core::intrinsics::abort() }
}
