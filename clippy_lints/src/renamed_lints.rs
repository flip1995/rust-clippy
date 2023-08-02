// This file is managed by `cargo dev rename_lint`. Prefer using that when possible.

#[rustfmt::skip]
pub static RENAMED_LINTS: &[(&str, &str)] = &[
    ("clippy::almost_complete_letter_range", "clippy::almost_complete_range"),
    ("clippy::blacklisted_name", "clippy::disallowed_names"),
    ("clippy::block_in_if_condition_expr", "clippy::blocks_in_if_conditions"),
    ("clippy::block_in_if_condition_stmt", "clippy::blocks_in_if_conditions"),
    ("clippy::box_vec", "clippy::box_collection"),
    ("clippy::const_static_lifetime", "clippy::redundant_static_lifetimes"),
    ("clippy::cyclomatic_complexity", "clippy::cognitive_complexity"),
    ("clippy::derive_hash_xor_eq", "clippy::derived_hash_with_manual_eq"),
    ("clippy::disallowed_method", "clippy::disallowed_methods"),
    ("clippy::disallowed_type", "clippy::disallowed_types"),
    ("clippy::eval_order_dependence", "clippy::mixed_read_write_in_expression"),
    ("clippy::identity_conversion", "clippy::useless_conversion"),
    ("clippy::if_let_some_result", "clippy::match_result_ok"),
    ("clippy::integer_arithmetic", "clippy::arithmetic_side_effects"),
    ("clippy::logic_bug", "clippy::overly_complex_bool_expr"),
    ("clippy::new_without_default_derive", "clippy::new_without_default"),
    ("clippy::option_and_then_some", "clippy::bind_instead_of_map"),
    ("clippy::option_expect_used", "clippy::expect_used"),
    ("clippy::option_map_unwrap_or", "clippy::map_unwrap_or"),
    ("clippy::option_map_unwrap_or_else", "clippy::map_unwrap_or"),
    ("clippy::option_unwrap_used", "clippy::unwrap_used"),
    ("clippy::ref_in_deref", "clippy::needless_borrow"),
    ("clippy::result_expect_used", "clippy::expect_used"),
    ("clippy::result_map_unwrap_or_else", "clippy::map_unwrap_or"),
    ("clippy::result_unwrap_used", "clippy::unwrap_used"),
    ("clippy::single_char_push_str", "clippy::single_char_add_str"),
    ("clippy::stutter", "clippy::module_name_repetitions"),
    ("clippy::to_string_in_display", "clippy::recursive_format_impl"),
    ("clippy::unwrap_or_else_default", "clippy::unwrap_or_default"),
    ("clippy::zero_width_space", "clippy::invisible_characters"),
    ("clippy::cast_ref_to_mut", "invalid_reference_casting"),
    ("clippy::clone_double_ref", "suspicious_double_ref_op"),
    ("clippy::cmp_nan", "invalid_nan_comparisons"),
    ("clippy::drop_bounds", "drop_bounds"),
    ("clippy::drop_copy", "dropping_copy_types"),
    ("clippy::drop_ref", "dropping_references"),
    ("clippy::for_loop_over_option", "for_loops_over_fallibles"),
    ("clippy::for_loop_over_result", "for_loops_over_fallibles"),
    ("clippy::for_loops_over_fallibles", "for_loops_over_fallibles"),
    ("clippy::forget_copy", "forgetting_copy_types"),
    ("clippy::forget_ref", "forgetting_references"),
    ("clippy::fn_null_check", "incorrect_fn_null_checks"),
    ("clippy::into_iter_on_array", "array_into_iter"),
    ("clippy::invalid_atomic_ordering", "invalid_atomic_ordering"),
    ("clippy::invalid_ref", "invalid_value"),
    ("clippy::invalid_utf8_in_unchecked", "invalid_from_utf8_unchecked"),
    ("clippy::let_underscore_drop", "let_underscore_drop"),
    ("clippy::mem_discriminant_non_enum", "enum_intrinsics_non_enums"),
    ("clippy::panic_params", "non_fmt_panics"),
    ("clippy::positional_named_format_parameters", "named_arguments_used_positionally"),
    ("clippy::temporary_cstring_as_ptr", "temporary_cstring_as_ptr"),
    ("clippy::undropped_manually_drops", "undropped_manually_drops"),
    ("clippy::unknown_clippy_lints", "unknown_lints"),
    ("clippy::unused_label", "unused_labels"),
];
