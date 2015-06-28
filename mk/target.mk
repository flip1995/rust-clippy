# Copyright 2012 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is the compile-time target-triple for the compiler. For the compiler at
# runtime, this should be considered the host-triple. More explanation for why
# this exists can be found on issue #2400
export CFG_COMPILER_HOST_TRIPLE

# The standard libraries should be held up to a higher standard than any old
# code, make sure that these common warnings are denied by default. These can
# be overridden during development temporarily. For stage0, we allow warnings
# which may be bugs in stage0 (should be fixed in stage1+)
RUST_LIB_FLAGS_ST0 += -W warnings
RUST_LIB_FLAGS_ST1 += -D warnings
RUST_LIB_FLAGS_ST2 += -D warnings

# Macro that generates the full list of dependencies for a crate at a particular
# stage/target/host tuple.
#
# $(1) - stage
# $(2) - target
# $(3) - host
# $(4) crate
define RUST_CRATE_FULLDEPS
CRATE_FULLDEPS_$(1)_T_$(2)_H_$(3)_$(4) := \
		$$(CRATEFILE_$(4)) \
		$$(RSINPUTS_$(4)) \
		$$(foreach dep,$$(RUST_DEPS_$(4)), \
		  $$(TLIB$(1)_T_$(2)_H_$(3))/stamp.$$(dep)) \
		$$(foreach dep,$$(NATIVE_DEPS_$(4)), \
		  $$(RT_OUTPUT_DIR_$(2))/$$(call CFG_STATIC_LIB_NAME_$(2),$$(dep))) \
		$$(foreach dep,$$(NATIVE_DEPS_$(4)_T_$(2)), \
		  $$(RT_OUTPUT_DIR_$(2))/$$(dep)) \
		$$(foreach dep,$$(NATIVE_TOOL_DEPS_$(4)_T_$(2)), \
		  $$(TBIN$(1)_T_$(3)_H_$(3))/$$(dep)) \
		$$(CUSTOM_DEPS_$(4)_T_$(2))
endef

$(foreach host,$(CFG_HOST), \
 $(foreach target,$(CFG_TARGET), \
  $(foreach stage,$(STAGES), \
   $(foreach crate,$(CRATES), \
    $(eval $(call RUST_CRATE_FULLDEPS,$(stage),$(target),$(host),$(crate)))))))

# RUST_TARGET_STAGE_N template: This defines how target artifacts are built
# for all stage/target architecture combinations. This is one giant rule which
# works as follows:
#
#   1. The immediate dependencies are the rust source files
#   2. Each rust crate dependency is listed (based on their stamp files),
#      as well as all native dependencies (listed in RT_OUTPUT_DIR)
#   3. The stage (n-1) compiler is required through the TSREQ dependency, along
#      with the morestack library
#   4. When actually executing the rule, the first thing we do is to clean out
#      old libs and rlibs via the REMOVE_ALL_OLD_GLOB_MATCHES macro
#   5. Finally, we get around to building the actual crate. It's just one
#      "small" invocation of the previous stage rustc. We use -L to
#      RT_OUTPUT_DIR so all the native dependencies are picked up.
#      Additionally, we pass in the llvm dir so rustc can link against it.
#   6. Some cleanup is done (listing what was just built) if verbose is turned
#      on.
#
# $(1) is the stage
# $(2) is the target triple
# $(3) is the host triple
# $(4) is the crate name
define RUST_TARGET_STAGE_N

$$(TLIB$(1)_T_$(2)_H_$(3))/stamp.$(4): CFG_COMPILER_HOST_TRIPLE = $(2)
$$(TLIB$(1)_T_$(2)_H_$(3))/stamp.$(4): \
		$$(CRATEFILE_$(4)) \
		$$(CRATE_FULLDEPS_$(1)_T_$(2)_H_$(3)_$(4)) \
		$$(LLVM_CONFIG_$(2)) \
		$$(TSREQ$(1)_T_$(2)_H_$(3)) \
		| $$(TLIB$(1)_T_$(2)_H_$(3))/
	@$$(call E, rustc: $$(@D)/lib$(4))
	@touch $$@.start_time
	$$(call REMOVE_ALL_OLD_GLOB_MATCHES, \
	    $$(dir $$@)$$(call CFG_LIB_GLOB_$(2),$(4)))
	$$(call REMOVE_ALL_OLD_GLOB_MATCHES, \
	    $$(dir $$@)$$(call CFG_RLIB_GLOB,$(4)))
	$(Q)CFG_LLVM_LINKAGE_FILE=$$(LLVM_LINKAGE_PATH_$(2)) \
	    $$(subst @,,$$(STAGE$(1)_T_$(2)_H_$(3))) \
		$$(RUST_LIB_FLAGS_ST$(1)) \
		-L "$$(RT_OUTPUT_DIR_$(2))" \
		$$(LLVM_LIBDIR_RUSTFLAGS_$(2)) \
		$$(LLVM_STDCPP_RUSTFLAGS_$(2)) \
		$$(RUSTFLAGS_$(4)) \
		$$(RUSTFLAGS_$(4)_T_$(2)) \
		--out-dir $$(@D) \
		-C extra-filename=-$$(CFG_FILENAME_EXTRA) \
		$$<
	@touch -r $$@.start_time $$@ && rm $$@.start_time
	$$(call LIST_ALL_OLD_GLOB_MATCHES, \
	    $$(dir $$@)$$(call CFG_LIB_GLOB_$(2),$(4)))
	$$(call LIST_ALL_OLD_GLOB_MATCHES, \
	    $$(dir $$@)$$(call CFG_RLIB_GLOB,$(4)))

endef

# Macro for building any tool as part of the rust compilation process. Each
# tool is defined in crates.mk with a list of library dependencies as well as
# the source file for the tool. Building each tool will also be passed '--cfg
# <tool>' for usage in driver.rs
#
# This build rule is similar to the one found above, just tweaked for
# locations and things.
#
# $(1) - stage
# $(2) - target triple
# $(3) - host triple
# $(4) - name of the tool being built
define TARGET_TOOL

$$(TBIN$(1)_T_$(2)_H_$(3))/$(4)$$(X_$(2)): \
		$$(TOOL_SOURCE_$(4)) \
		$$(TOOL_INPUTS_$(4)) \
		$$(foreach dep,$$(TOOL_DEPS_$(4)), \
		    $$(TLIB$(1)_T_$(2)_H_$(3))/stamp.$$(dep)) \
		$$(TSREQ$(1)_T_$(2)_H_$(3)) \
		| $$(TBIN$(1)_T_$(2)_H_$(3))/
	@$$(call E, rustc: $$@)
	$$(STAGE$(1)_T_$(2)_H_$(3)) -o $$@ $$< --cfg $(4)

endef

# Every recipe in RUST_TARGET_STAGE_N outputs to $$(TLIB$(1)_T_$(2)_H_$(3),
# a directory that can be cleaned out during the middle of a run of
# the get-snapshot.py script.  Therefore, every recipe needs to have
# an order-only dependency either on $(SNAPSHOT_RUSTC_POST_CLEANUP) or
# on $$(TSREQ$(1)_T_$(2)_H_$(3)), to ensure that no products will be
# put into the target area until after the get-snapshot.py script has
# had its chance to clean it out; otherwise the other products will be
# inadvertently included in the clean out.
SNAPSHOT_RUSTC_POST_CLEANUP=$(HBIN0_H_$(CFG_BUILD))/rustc$(X_$(CFG_BUILD))

define TARGET_HOST_RULES

$$(TBIN$(1)_T_$(2)_H_$(3))/:
	mkdir -p $$@

$$(TLIB$(1)_T_$(2)_H_$(3))/:
	mkdir -p $$@

$$(TLIB$(1)_T_$(2)_H_$(3))/%: $$(RT_OUTPUT_DIR_$(2))/% \
	    | $$(TLIB$(1)_T_$(2)_H_$(3))/ $$(SNAPSHOT_RUSTC_POST_CLEANUP)
	@$$(call E, cp: $$@)
	$$(Q)cp $$< $$@

$$(TBIN$(1)_T_$(2)_H_$(3))/%: $$(CFG_LLVM_INST_DIR_$(2))/bin/% \
	    | $$(TBIN$(1)_T_$(2)_H_$(3))/ $$(SNAPSHOT_RUSTC_POST_CLEANUP)
	@$$(call E, cp: $$@)
	$$(Q)cp $$< $$@
endef

$(foreach source,$(CFG_HOST), \
 $(foreach target,$(CFG_TARGET), \
  $(eval $(call TARGET_HOST_RULES,0,$(target),$(source))) \
  $(eval $(call TARGET_HOST_RULES,1,$(target),$(source))) \
  $(eval $(call TARGET_HOST_RULES,2,$(target),$(source))) \
  $(eval $(call TARGET_HOST_RULES,3,$(target),$(source)))))

# In principle, each host can build each target for both libs and tools
$(foreach crate,$(CRATES), \
 $(foreach source,$(CFG_HOST), \
  $(foreach target,$(CFG_TARGET), \
   $(eval $(call RUST_TARGET_STAGE_N,0,$(target),$(source),$(crate))) \
   $(eval $(call RUST_TARGET_STAGE_N,1,$(target),$(source),$(crate))) \
   $(eval $(call RUST_TARGET_STAGE_N,2,$(target),$(source),$(crate))) \
   $(eval $(call RUST_TARGET_STAGE_N,3,$(target),$(source),$(crate))))))

$(foreach host,$(CFG_HOST), \
 $(foreach target,$(CFG_TARGET), \
  $(foreach stage,$(STAGES), \
   $(foreach tool,$(TOOLS), \
    $(eval $(call TARGET_TOOL,$(stage),$(target),$(host),$(tool)))))))

# We have some triples which are bootstrapped from other triples, and this means
# that we need to fixup some of the native tools that a triple depends on.
#
# For example, MSVC requires the llvm-ar.exe executable to manage archives, but
# it bootstraps from the GNU Windows triple. This means that the compiler will
# add this directory to PATH when executing new processes:
#
# 	$SYSROOT/rustlib/x86_64-pc-windows-gnu/bin
#
# Unfortunately, however, the GNU triple is not known about in stage0, so the
# tools are actually located in:
#
# 	$SYSROOT/rustlib/x86_64-pc-windows-msvc/bin
#
# To remedy this problem, the rules below copy all native tool dependencies into
# the bootstrap triple's location in stage 0 so the bootstrap compiler can find
# the right sets of tools. Later stages (1+) will have the right host triple for
# the compiler, so there's no need to worry there.
#
# $(1) - stage
# $(2) - triple that's being used as host/target
# $(3) - triple snapshot is built for
# $(4) - crate
# $(5) - tool
define MOVE_TOOLS_TO_SNAPSHOT_HOST_DIR
ifneq (,$(3))
$$(TLIB$(1)_T_$(2)_H_$(2))/stamp.$(4): $$(HLIB$(1)_H_$(2))/rustlib/$(3)/bin/$(5)

$$(HLIB$(1)_H_$(2))/rustlib/$(3)/bin/$(5): $$(TBIN$(1)_T_$(2)_H_$(2))/$(5)
	mkdir -p $$(@D)
	cp $$< $$@
endif
endef

$(foreach target,$(CFG_TARGET), \
 $(foreach crate,$(CRATES), \
  $(foreach tool,$(NATIVE_TOOL_DEPS_$(crate)_T_$(target)), \
   $(eval $(call MOVE_TOOLS_TO_SNAPSHOT_HOST_DIR,0,$(target),$(BOOTSTRAP_FROM_$(target)),$(crate),$(tool))))))

# For MSVC targets we need to set up some environment variables for the linker
# to work correctly when building Rust crates. These two variables are:
#
# - LIB tells the linker the default search path for finding system libraries,
#   for example kernel32.dll
# - PATH needs to be modified to ensure that MSVC's link.exe is first in the
#   path instead of MinGW's /usr/bin/link.exe (entirely unrelated)
#
# The values for these variables are detected by the configure script.
define SETUP_LIB_MSVC_ENV_VARS
ifeq ($$(findstring msvc,$(2)),msvc)
$$(TLIB$(1)_T_$(2)_H_$(3))/stamp.$(4): \
	export LIB := $$(CFG_MSVC_LIB_PATH_$$(HOST_$(2)))
$$(TLIB$(1)_T_$(2)_H_$(3))/stamp.$(4): \
	export PATH := $$(CFG_MSVC_BINDIR_$$(HOST_$(2))):$$(PATH)
endif
endef
define SETUP_TOOL_MSVC_ENV_VARS
ifeq ($$(findstring msvc,$(2)),msvc)
$$(TBIN$(1)_T_$(2)_H_$(3))/$(4)$$(X_$(2)): \
	export LIB := $$(CFG_MSVC_LIB_PATH_$$(HOST_$(2)))
$$(TBIN$(1)_T_$(2)_H_$(3))/$(4)$$(X_$(2)): \
	export PATH := $$(CFG_MSVC_BINDIR_$$(HOST_$(2))):$$(PATH)
endif
endef

$(foreach host,$(CFG_HOST), \
 $(foreach target,$(CFG_TARGET), \
  $(foreach stage,$(STAGES), \
   $(foreach crate,$(CRATES), \
    $(eval $(call SETUP_LIB_MSVC_ENV_VARS,$(stage),$(target),$(host),$(crate)))))))
$(foreach host,$(CFG_HOST), \
 $(foreach target,$(CFG_TARGET), \
  $(foreach stage,$(STAGES), \
   $(foreach tool,$(TOOLS), \
    $(eval $(call SETUP_TOOL_MSVC_ENV_VARS,$(stage),$(target),$(host),$(tool)))))))
