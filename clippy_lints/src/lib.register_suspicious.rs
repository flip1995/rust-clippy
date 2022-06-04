// This file was generated by `cargo dev update_lints`.
// Use that command to update this file and do not edit by hand.
// Manual edits will be overwritten.

store.register_group(true, "clippy::suspicious", Some("clippy_suspicious"), vec![
    LintId::of(almost_complete_letter_range::ALMOST_COMPLETE_LETTER_RANGE),
    LintId::of(assign_ops::MISREFACTORED_ASSIGN_OP),
    LintId::of(attrs::BLANKET_CLIPPY_RESTRICTION_LINTS),
    LintId::of(await_holding_invalid::AWAIT_HOLDING_INVALID_TYPE),
    LintId::of(await_holding_invalid::AWAIT_HOLDING_LOCK),
    LintId::of(await_holding_invalid::AWAIT_HOLDING_REFCELL_REF),
    LintId::of(casts::CAST_ABS_TO_UNSIGNED),
    LintId::of(casts::CAST_ENUM_CONSTRUCTOR),
    LintId::of(casts::CAST_ENUM_TRUNCATION),
    LintId::of(crate_in_macro_def::CRATE_IN_MACRO_DEF),
    LintId::of(drop_forget_ref::DROP_NON_DROP),
    LintId::of(drop_forget_ref::FORGET_NON_DROP),
    LintId::of(duplicate_mod::DUPLICATE_MOD),
    LintId::of(float_equality_without_abs::FLOAT_EQUALITY_WITHOUT_ABS),
    LintId::of(format_impl::PRINT_IN_FORMAT_IMPL),
    LintId::of(formatting::SUSPICIOUS_ASSIGNMENT_FORMATTING),
    LintId::of(formatting::SUSPICIOUS_ELSE_FORMATTING),
    LintId::of(formatting::SUSPICIOUS_UNARY_OP_FORMATTING),
    LintId::of(loops::EMPTY_LOOP),
    LintId::of(loops::FOR_LOOPS_OVER_FALLIBLES),
    LintId::of(loops::MUT_RANGE_BOUND),
    LintId::of(matches::SIGNIFICANT_DROP_IN_SCRUTINEE),
    LintId::of(methods::NO_EFFECT_REPLACE),
    LintId::of(methods::SUSPICIOUS_MAP),
    LintId::of(mut_key::MUTABLE_KEY_TYPE),
    LintId::of(octal_escapes::OCTAL_ESCAPES),
    LintId::of(rc_clone_in_vec_init::RC_CLONE_IN_VEC_INIT),
    LintId::of(suspicious_trait_impl::SUSPICIOUS_ARITHMETIC_IMPL),
    LintId::of(suspicious_trait_impl::SUSPICIOUS_OP_ASSIGN_IMPL),
    LintId::of(swap_ptr_to_ref::SWAP_PTR_TO_REF),
])
