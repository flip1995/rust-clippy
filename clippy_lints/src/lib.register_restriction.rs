// This file was generated by `cargo dev update_lints`.
// Use that command to update this file and do not edit by hand.
// Manual edits will be overwritten.

store.register_group(true, "clippy::restriction", Some("clippy_restriction"), vec![
    LintId::of(as_conversions::AS_CONVERSIONS),
    LintId::of(asm_syntax::INLINE_ASM_X86_ATT_SYNTAX),
    LintId::of(asm_syntax::INLINE_ASM_X86_INTEL_SYNTAX),
    LintId::of(assertions_on_result_states::ASSERTIONS_ON_RESULT_STATES),
    LintId::of(attrs::ALLOW_ATTRIBUTES_WITHOUT_REASON),
    LintId::of(casts::AS_UNDERSCORE),
    LintId::of(casts::FN_TO_NUMERIC_CAST_ANY),
    LintId::of(create_dir::CREATE_DIR),
    LintId::of(dbg_macro::DBG_MACRO),
    LintId::of(default_numeric_fallback::DEFAULT_NUMERIC_FALLBACK),
    LintId::of(default_union_representation::DEFAULT_UNION_REPRESENTATION),
    LintId::of(disallowed_script_idents::DISALLOWED_SCRIPT_IDENTS),
    LintId::of(else_if_without_else::ELSE_IF_WITHOUT_ELSE),
    LintId::of(empty_drop::EMPTY_DROP),
    LintId::of(empty_structs_with_brackets::EMPTY_STRUCTS_WITH_BRACKETS),
    LintId::of(exhaustive_items::EXHAUSTIVE_ENUMS),
    LintId::of(exhaustive_items::EXHAUSTIVE_STRUCTS),
    LintId::of(exit::EXIT),
    LintId::of(float_literal::LOSSY_FLOAT_LITERAL),
    LintId::of(format_push_string::FORMAT_PUSH_STRING),
    LintId::of(if_then_some_else_none::IF_THEN_SOME_ELSE_NONE),
    LintId::of(implicit_return::IMPLICIT_RETURN),
    LintId::of(indexing_slicing::INDEXING_SLICING),
    LintId::of(inherent_impl::MULTIPLE_INHERENT_IMPL),
    LintId::of(large_include_file::LARGE_INCLUDE_FILE),
    LintId::of(let_underscore::LET_UNDERSCORE_MUST_USE),
    LintId::of(literal_representation::DECIMAL_LITERAL_REPRESENTATION),
    LintId::of(matches::REST_PAT_IN_FULLY_BOUND_STRUCTS),
    LintId::of(matches::TRY_ERR),
    LintId::of(matches::WILDCARD_ENUM_MATCH_ARM),
    LintId::of(mem_forget::MEM_FORGET),
    LintId::of(methods::CLONE_ON_REF_PTR),
    LintId::of(methods::EXPECT_USED),
    LintId::of(methods::FILETYPE_IS_FILE),
    LintId::of(methods::GET_UNWRAP),
    LintId::of(methods::MAP_ERR_IGNORE),
    LintId::of(methods::UNWRAP_USED),
    LintId::of(methods::VERBOSE_FILE_READS),
    LintId::of(misc_early::SEPARATED_LITERAL_SUFFIX),
    LintId::of(misc_early::UNNEEDED_FIELD_PATTERN),
    LintId::of(misc_early::UNSEPARATED_LITERAL_SUFFIX),
    LintId::of(missing_doc::MISSING_DOCS_IN_PRIVATE_ITEMS),
    LintId::of(missing_enforced_import_rename::MISSING_ENFORCED_IMPORT_RENAMES),
    LintId::of(missing_inline::MISSING_INLINE_IN_PUBLIC_ITEMS),
    LintId::of(mixed_read_write_in_expression::MIXED_READ_WRITE_IN_EXPRESSION),
    LintId::of(module_style::MOD_MODULE_FILES),
    LintId::of(module_style::SELF_NAMED_MODULE_FILES),
    LintId::of(operators::ARITHMETIC_SIDE_EFFECTS),
    LintId::of(operators::FLOAT_ARITHMETIC),
    LintId::of(operators::FLOAT_CMP_CONST),
    LintId::of(operators::INTEGER_ARITHMETIC),
    LintId::of(operators::INTEGER_DIVISION),
    LintId::of(operators::MODULO_ARITHMETIC),
    LintId::of(panic_in_result_fn::PANIC_IN_RESULT_FN),
    LintId::of(panic_unimplemented::PANIC),
    LintId::of(panic_unimplemented::TODO),
    LintId::of(panic_unimplemented::UNIMPLEMENTED),
    LintId::of(panic_unimplemented::UNREACHABLE),
    LintId::of(pattern_type_mismatch::PATTERN_TYPE_MISMATCH),
    LintId::of(pub_use::PUB_USE),
    LintId::of(redundant_slicing::DEREF_BY_SLICING),
    LintId::of(same_name_method::SAME_NAME_METHOD),
    LintId::of(shadow::SHADOW_REUSE),
    LintId::of(shadow::SHADOW_SAME),
    LintId::of(shadow::SHADOW_UNRELATED),
    LintId::of(single_char_lifetime_names::SINGLE_CHAR_LIFETIME_NAMES),
    LintId::of(std_instead_of_core::ALLOC_INSTEAD_OF_CORE),
    LintId::of(std_instead_of_core::STD_INSTEAD_OF_ALLOC),
    LintId::of(std_instead_of_core::STD_INSTEAD_OF_CORE),
    LintId::of(strings::STRING_ADD),
    LintId::of(strings::STRING_SLICE),
    LintId::of(strings::STRING_TO_STRING),
    LintId::of(strings::STR_TO_STRING),
    LintId::of(types::RC_BUFFER),
    LintId::of(types::RC_MUTEX),
    LintId::of(undocumented_unsafe_blocks::UNDOCUMENTED_UNSAFE_BLOCKS),
    LintId::of(unicode::NON_ASCII_LITERAL),
    LintId::of(unnecessary_self_imports::UNNECESSARY_SELF_IMPORTS),
    LintId::of(unwrap_in_result::UNWRAP_IN_RESULT),
    LintId::of(write::PRINT_STDERR),
    LintId::of(write::PRINT_STDOUT),
    LintId::of(write::USE_DEBUG),
])
