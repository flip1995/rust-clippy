// This file was generated by `cargo dev update_lints`.
// Use that command to update this file and do not edit by hand.
// Manual edits will be overwritten.

store.register_group(true, "clippy::correctness", Some("clippy_correctness"), vec![
    LintId::of(absurd_extreme_comparisons::ABSURD_EXTREME_COMPARISONS),
    LintId::of(approx_const::APPROX_CONSTANT),
    LintId::of(async_yields_async::ASYNC_YIELDS_ASYNC),
    LintId::of(attrs::DEPRECATED_SEMVER),
    LintId::of(attrs::MISMATCHED_TARGET_OS),
    LintId::of(attrs::USELESS_ATTRIBUTE),
    LintId::of(bit_mask::BAD_BIT_MASK),
    LintId::of(bit_mask::INEFFECTIVE_BIT_MASK),
    LintId::of(booleans::LOGIC_BUG),
    LintId::of(casts::CAST_REF_TO_MUT),
    LintId::of(copies::IFS_SAME_COND),
    LintId::of(copies::IF_SAME_THEN_ELSE),
    LintId::of(derive::DERIVE_HASH_XOR_EQ),
    LintId::of(derive::DERIVE_ORD_XOR_PARTIAL_ORD),
    LintId::of(drop_forget_ref::DROP_COPY),
    LintId::of(drop_forget_ref::DROP_REF),
    LintId::of(drop_forget_ref::FORGET_COPY),
    LintId::of(drop_forget_ref::FORGET_REF),
    LintId::of(enum_clike::ENUM_CLIKE_UNPORTABLE_VARIANT),
    LintId::of(eq_op::EQ_OP),
    LintId::of(erasing_op::ERASING_OP),
    LintId::of(formatting::POSSIBLE_MISSING_COMMA),
    LintId::of(functions::NOT_UNSAFE_PTR_ARG_DEREF),
    LintId::of(if_let_mutex::IF_LET_MUTEX),
    LintId::of(indexing_slicing::OUT_OF_BOUNDS_INDEXING),
    LintId::of(infinite_iter::INFINITE_ITER),
    LintId::of(inherent_to_string::INHERENT_TO_STRING_SHADOW_DISPLAY),
    LintId::of(inline_fn_without_body::INLINE_FN_WITHOUT_BODY),
    LintId::of(let_underscore::LET_UNDERSCORE_LOCK),
    LintId::of(literal_representation::MISTYPED_LITERAL_SUFFIXES),
    LintId::of(loops::ITER_NEXT_LOOP),
    LintId::of(loops::NEVER_LOOP),
    LintId::of(loops::WHILE_IMMUTABLE_CONDITION),
    LintId::of(mem_discriminant::MEM_DISCRIMINANT_NON_ENUM),
    LintId::of(mem_replace::MEM_REPLACE_WITH_UNINIT),
    LintId::of(methods::CLONE_DOUBLE_REF),
    LintId::of(methods::ITERATOR_STEP_BY_ZERO),
    LintId::of(methods::SUSPICIOUS_SPLITN),
    LintId::of(methods::UNINIT_ASSUMED_INIT),
    LintId::of(methods::ZST_OFFSET),
    LintId::of(minmax::MIN_MAX),
    LintId::of(misc::CMP_NAN),
    LintId::of(misc::MODULO_ONE),
    LintId::of(non_octal_unix_permissions::NON_OCTAL_UNIX_PERMISSIONS),
    LintId::of(open_options::NONSENSICAL_OPEN_OPTIONS),
    LintId::of(option_env_unwrap::OPTION_ENV_UNWRAP),
    LintId::of(ptr::INVALID_NULL_PTR_USAGE),
    LintId::of(ptr::MUT_FROM_REF),
    LintId::of(ranges::REVERSED_EMPTY_RANGES),
    LintId::of(regex::INVALID_REGEX),
    LintId::of(self_assignment::SELF_ASSIGNMENT),
    LintId::of(serde_api::SERDE_API_MISUSE),
    LintId::of(size_of_in_element_count::SIZE_OF_IN_ELEMENT_COUNT),
    LintId::of(swap::ALMOST_SWAPPED),
    LintId::of(to_string_in_display::TO_STRING_IN_DISPLAY),
    LintId::of(transmute::UNSOUND_COLLECTION_TRANSMUTE),
    LintId::of(transmute::WRONG_TRANSMUTE),
    LintId::of(transmuting_null::TRANSMUTING_NULL),
    LintId::of(undropped_manually_drops::UNDROPPED_MANUALLY_DROPS),
    LintId::of(unicode::INVISIBLE_CHARACTERS),
    LintId::of(unit_return_expecting_ord::UNIT_RETURN_EXPECTING_ORD),
    LintId::of(unit_types::UNIT_CMP),
    LintId::of(unnamed_address::FN_ADDRESS_COMPARISONS),
    LintId::of(unnamed_address::VTABLE_ADDRESS_COMPARISONS),
    LintId::of(unused_io_amount::UNUSED_IO_AMOUNT),
    LintId::of(unwrap::PANICKING_UNWRAP),
    LintId::of(vec_resize_to_zero::VEC_RESIZE_TO_ZERO),
])
