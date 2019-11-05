// FIXME: The assumes we're using the non-vector ABI, i.e., compiling
// for a pre-z13 machine or using -mno-vx.

use crate::abi::call::{FnAbi, ArgAbi, Reg};
use crate::abi::{self, HasDataLayout, LayoutOf, TyLayout, TyLayoutMethods};

fn classify_ret<'a, Ty, C>(ret: &mut ArgAbi<'_, Ty>)
    where Ty: TyLayoutMethods<'a, C>, C: LayoutOf<Ty = Ty> + HasDataLayout
{
    if !ret.layout.is_aggregate() && ret.layout.size.bits() <= 64 {
        ret.extend_integer_width_to(64);
    } else {
        ret.make_indirect();
    }
}

fn is_single_fp_element<'a, Ty, C>(cx: &C, layout: TyLayout<'a, Ty>) -> bool
    where Ty: TyLayoutMethods<'a, C>,
          C: LayoutOf<Ty = Ty, TyLayout = TyLayout<'a, Ty>> + HasDataLayout
{
    match layout.abi {
        abi::Abi::Scalar(ref scalar) => scalar.value.is_float(),
        abi::Abi::Aggregate { .. } => {
            if layout.fields.count() == 1 && layout.fields.offset(0).bytes() == 0 {
                is_single_fp_element(cx, layout.field(cx, 0))
            } else {
                false
            }
        }
        _ => false
    }
}

fn classify_arg<'a, Ty, C>(cx: &C, arg: &mut ArgAbi<'a, Ty>)
    where Ty: TyLayoutMethods<'a, C> + Copy,
          C: LayoutOf<Ty = Ty, TyLayout = TyLayout<'a, Ty>> + HasDataLayout
{
    if !arg.layout.is_aggregate() && arg.layout.size.bits() <= 64 {
        arg.extend_integer_width_to(64);
        return;
    }

    if is_single_fp_element(cx, arg.layout) {
        match arg.layout.size.bytes() {
            4 => arg.cast_to(Reg::f32()),
            8 => arg.cast_to(Reg::f64()),
            _ => arg.make_indirect()
        }
    } else {
        match arg.layout.size.bytes() {
            1 => arg.cast_to(Reg::i8()),
            2 => arg.cast_to(Reg::i16()),
            4 => arg.cast_to(Reg::i32()),
            8 => arg.cast_to(Reg::i64()),
            _ => arg.make_indirect()
        }
    }
}

pub fn compute_abi_info<'a, Ty, C>(cx: &C, fn_abi: &mut FnAbi<'a, Ty>)
    where Ty: TyLayoutMethods<'a, C> + Copy,
          C: LayoutOf<Ty = Ty, TyLayout = TyLayout<'a, Ty>> + HasDataLayout
{
    if !fn_abi.ret.is_ignore() {
        classify_ret(&mut fn_abi.ret);
    }

    for arg in &mut fn_abi.args {
        if arg.is_ignore() { continue; }
        classify_arg(cx, arg);
    }
}
