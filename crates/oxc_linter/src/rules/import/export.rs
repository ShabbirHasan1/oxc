use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::{self, Error},
};
use oxc_macros::declare_oxc_lint;
use oxc_semantic::ModuleRecord;
use oxc_span::{Atom, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{context::LintContext, rule::Rule};

#[derive(Debug, Error, Diagnostic)]
enum ExportDiagnostic {
    #[error("eslint-plugin-import(export): Multiple exports of name '{1}'.")]
    #[diagnostic(severity(warning))]
    NamedExport(#[label] Span, Atom),
}

/// <https://github.com/import-js/eslint-plugin-import/blob/main/docs/rules/export.md>
#[derive(Debug, Default, Clone)]
pub struct Export;

declare_oxc_lint!(
    /// ### What it does
    /// Reports funny business with exports, like repeated exports of names or defaults.
    ///
    /// ### Example
    /// ```javascript
    /// let foo;
    /// export { foo }; // Multiple exports of name 'foo'.
    /// export * from "./export-all" // export-all.js also export foo
    /// ```
    Export,
    restriction
);

impl Rule for Export {
    fn run_once(&self, ctx: &LintContext<'_>) {
        let module_record = ctx.semantic().module_record();
        let named_export = &module_record.exported_bindings;
        let mut duplicated_named_export = FxHashMap::default();
        if module_record.star_export_entries.is_empty() {
            return;
        }
        for export_entry in &module_record.star_export_entries {
            let Some(module_request) = &export_entry.module_request else {
                continue;
            };
            let Some(remote_module_record_ref) =
                module_record.loaded_modules.get(module_request.name())
            else {
                continue;
            };

            let remote_module_record = remote_module_record_ref.value();
            let mut all_export_names = FxHashSet::default();
            collect_exported_recursive(&mut all_export_names, remote_module_record);
            for name in &all_export_names {
                if let Some(span) = named_export.get(name) {
                    duplicated_named_export.entry(*span).or_insert_with(|| name.clone());
                }
            }
        }

        for (span, name) in duplicated_named_export {
            ctx.diagnostic(ExportDiagnostic::NamedExport(span, name));
        }
    }
}

// TODO: support detect cycle
fn collect_exported_recursive(result: &mut FxHashSet<Atom>, module_record: &ModuleRecord) {
    for name in module_record.exported_bindings.keys() {
        result.insert(name.clone());
    }
    for export_entry in &module_record.star_export_entries {
        let Some(module_request) = &export_entry.module_request else {
            continue;
        };
        let Some(remote_module_record_ref) =
            module_record.loaded_modules.get(module_request.name())
        else {
            continue;
        };
        collect_exported_recursive(result, remote_module_record_ref.value());
    }
}

#[test]
fn test() {
    use crate::tester::Tester;
    use serde_json::Value;

    let pass: Vec<(&str, Option<Value>)> = vec![(r#"var foo = "foo"; export default foo;"#, None)];

    let fail = vec![(r#"let foo; export { foo }; export * from "./export-all""#, None)];

    Tester::new(Export::NAME, pass, fail)
        .change_rule_path("index.js")
        .with_import_plugin(true)
        .test_and_snapshot();
}
