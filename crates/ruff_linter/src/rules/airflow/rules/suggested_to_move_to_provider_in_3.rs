use crate::checkers::ast::Checker;
use crate::rules::airflow::helpers::{
    ProviderReplacement, generate_import_edit, generate_remove_and_runtime_import_edit,
    is_guarded_by_try_except,
};
use crate::{FixAvailability, Violation};
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{Expr, ExprAttribute};
use ruff_python_semantic::Modules;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

/// ## What it does
/// Checks for uses of Airflow functions and values that have been moved to its providers
/// but still have a compatibility layer (e.g., `apache-airflow-providers-standard`).
///
/// ## Why is this bad?
/// Airflow 3.0 moved various deprecated functions, members, and other
/// values to its providers. Even though these symbols still work fine on Airflow 3.0,
/// they are expected to be removed in a future version. The user is suggested to install
/// the corresponding provider and replace the original usage with the one in the provider.
///
/// ## Example
/// ```python
/// from airflow.operators.python import PythonOperator
///
///
/// def print_context(ds=None, **kwargs):
///     print(kwargs)
///     print(ds)
///
///
/// print_the_context = PythonOperator(
///     task_id="print_the_context", python_callable=print_context
/// )
/// ```
///
/// Use instead:
/// ```python
/// from airflow.providers.standard.operators.python import PythonOperator
///
///
/// def print_context(ds=None, **kwargs):
///     print(kwargs)
///     print(ds)
///
///
/// print_the_context = PythonOperator(
///     task_id="print_the_context", python_callable=print_context
/// )
/// ```
#[derive(ViolationMetadata)]
pub(crate) struct Airflow3SuggestedToMoveToProvider<'a> {
    deprecated: QualifiedName<'a>,
    replacement: ProviderReplacement,
}

impl Violation for Airflow3SuggestedToMoveToProvider<'_> {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;
    #[derive_message_formats]
    fn message(&self) -> String {
        let Airflow3SuggestedToMoveToProvider {
            deprecated,
            replacement,
        } = self;
        match replacement {
            ProviderReplacement::None => {
                format!("`{deprecated}` is removed in Airflow 3.0")
            }
            ProviderReplacement::AutoImport {
                name: _,
                module: _,
                provider,
                version: _,
            }
            | ProviderReplacement::SourceModuleMovedToProvider {
                name: _,
                module: _,
                provider,
                version: _,
            } => {
                format!(
                    "`{deprecated}` is deprecated and moved into `{provider}` provider in Airflow 3.0; \
                     It still works in Airflow 3.0 but is expected to be removed in a future version."
                )
            }
        }
    }

    fn fix_title(&self) -> Option<String> {
        let Airflow3SuggestedToMoveToProvider { replacement, .. } = self;
        match replacement {
            ProviderReplacement::None => None,
            ProviderReplacement::AutoImport {
                module,
                name,
                provider,
                version,
            } => Some(format!(
                "Install `apache-airflow-providers-{provider}>={version}` and use `{name}` from `{module}` instead."
            )),
            ProviderReplacement::SourceModuleMovedToProvider {
                module,
                name,
                provider,
                version,
            } => Some(format!(
                "Install `apache-airflow-providers-{provider}>={version}` and use `{name}` from `{module}` instead."
            )),
        }
    }
}

// AIR312
pub(crate) fn suggested_to_move_to_provider_in_3(checker: &Checker, expr: &Expr) {
    if !checker.semantic().seen_module(Modules::AIRFLOW) {
        return;
    }

    match expr {
        Expr::Attribute(ExprAttribute { attr, .. }) => {
            check_names_moved_to_provider(checker, expr, attr.range());
        }
        Expr::Name(_) => check_names_moved_to_provider(checker, expr, expr.range()),
        _ => {}
    }
}

fn check_names_moved_to_provider(checker: &Checker, expr: &Expr, ranged: TextRange) {
    let Some(qualified_name) = checker.semantic().resolve_qualified_name(expr) else {
        return;
    };

    let replacement = match qualified_name.segments() {
        // apache-airflow-providers-standard
        ["airflow", "hooks", "filesystem", "FSHook"] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.hooks.filesystem",
            name: "FSHook",
            provider: "standard",
            version: "0.0.1",
        },
        ["airflow", "hooks", "package_index", "PackageIndexHook"] => {
            ProviderReplacement::AutoImport {
                module: "airflow.providers.standard.hooks.package_index",
                name: "PackageIndexHook",
                provider: "standard",
                version: "0.0.1",
            }
        }
        ["airflow", "hooks", "subprocess", "SubprocessHook"] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.hooks.subprocess",
            name: "SubprocessHook",
            provider: "standard",
            version: "0.0.3",
        },
        ["airflow", "operators", "bash", "BashOperator"] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.operators.bash",
            name: "BashOperator",
            provider: "standard",
            version: "0.0.1",
        },
        ["airflow", "operators", "datetime", "BranchDateTimeOperator"] => {
            ProviderReplacement::AutoImport {
                module: "airflow.providers.standard.operators.datetime",
                name: "BranchDateTimeOperator",
                provider: "standard",
                version: "0.0.1",
            }
        }
        [
            "airflow",
            "operators",
            "trigger_dagrun",
            "TriggerDagRunOperator",
        ] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.operators.trigger_dagrun",
            name: "TriggerDagRunOperator",
            provider: "standard",
            version: "0.0.2",
        },
        ["airflow", "operators", "empty", "EmptyOperator"] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.operators.empty",
            name: "EmptyOperator",
            provider: "standard",
            version: "0.0.2",
        },
        ["airflow", "operators", "latest_only", "LatestOnlyOperator"] => {
            ProviderReplacement::AutoImport {
                module: "airflow.providers.standard.operators.latest_only",
                name: "LatestOnlyOperator",
                provider: "standard",
                version: "0.0.3",
            }
        }
        [
            "airflow",
            "operators",
            "python",
            rest @ ("BranchPythonOperator"
            | "PythonOperator"
            | "PythonVirtualenvOperator"
            | "ShortCircuitOperator"),
        ] => ProviderReplacement::SourceModuleMovedToProvider {
            name: (*rest).to_string(),
            module: "airflow.providers.standard.operators.python",
            provider: "standard",
            version: "0.0.1",
        },
        ["airflow", "operators", "weekday", "BranchDayOfWeekOperator"] => {
            ProviderReplacement::AutoImport {
                module: "airflow.providers.standard.operators.weekday",
                name: "BranchDayOfWeekOperator",
                provider: "standard",
                version: "0.0.1",
            }
        }
        [
            "airflow",
            "sensors",
            "date_time",
            rest @ ("DateTimeSensor" | "DateTimeSensorAsync"),
        ] => ProviderReplacement::SourceModuleMovedToProvider {
            name: (*rest).to_string(),
            module: "airflow.providers.standard.sensors.date_time",
            provider: "standard",
            version: "0.0.1",
        },
        [
            "airflow",
            "sensors",
            "external_task",
            rest @ ("ExternalTaskMarker" | "ExternalTaskSensor" | "ExternalTaskSensorLink"),
        ] => ProviderReplacement::SourceModuleMovedToProvider {
            name: (*rest).to_string(),
            module: "airflow.providers.standard.sensors.external_task",
            provider: "standard",
            version: "0.0.3",
        },
        ["airflow", "sensors", "filesystem", "FileSensor"] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.sensors.filesystem",
            name: "FileSensor",
            provider: "standard",
            version: "0.0.2",
        },
        [
            "airflow",
            "sensors",
            "time_sensor",
            rest @ ("TimeSensor" | "TimeSensorAsync"),
        ] => ProviderReplacement::SourceModuleMovedToProvider {
            name: (*rest).to_string(),
            module: "airflow.providers.standard.sensors.time",
            provider: "standard",
            version: "0.0.1",
        },
        [
            "airflow",
            "sensors",
            "time_delta",
            rest @ ("TimeDeltaSensor" | "TimeDeltaSensorAsync"),
        ] => ProviderReplacement::SourceModuleMovedToProvider {
            name: (*rest).to_string(),
            module: "airflow.providers.standard.sensors.time_delta",
            provider: "standard",
            version: "0.0.1",
        },
        ["airflow", "sensors", "weekday", "DayOfWeekSensor"] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.sensors.weekday",
            name: "DayOfWeekSensor",
            provider: "standard",
            version: "0.0.1",
        },
        [
            "airflow",
            "triggers",
            "external_task",
            rest @ ("DagStateTrigger" | "WorkflowTrigger"),
        ] => ProviderReplacement::SourceModuleMovedToProvider {
            name: (*rest).to_string(),
            module: "airflow.providers.standard.triggers.external_task",
            provider: "standard",
            version: "0.0.3",
        },
        ["airflow", "triggers", "file", "FileTrigger"] => ProviderReplacement::AutoImport {
            module: "airflow.providers.standard.triggers.file",
            name: "FileTrigger",
            provider: "standard",
            version: "0.0.3",
        },
        [
            "airflow",
            "triggers",
            "temporal",
            rest @ ("DateTimeTrigger" | "TimeDeltaTrigger"),
        ] => ProviderReplacement::SourceModuleMovedToProvider {
            name: (*rest).to_string(),
            module: "airflow.providers.standard.triggers.temporal",
            provider: "standard",
            version: "0.0.3",
        },
        _ => return,
    };

    let (module, name) = match &replacement {
        ProviderReplacement::AutoImport { module, name, .. } => (module, *name),
        ProviderReplacement::SourceModuleMovedToProvider { module, name, .. } => {
            (module, name.as_str())
        }
        ProviderReplacement::None => {
            checker.report_diagnostic(
                Airflow3SuggestedToMoveToProvider {
                    deprecated: qualified_name,
                    replacement: replacement.clone(),
                },
                ranged.range(),
            );
            return;
        }
    };

    if is_guarded_by_try_except(expr, module, name, checker.semantic()) {
        return;
    }
    let mut diagnostic = checker.report_diagnostic(
        Airflow3SuggestedToMoveToProvider {
            deprecated: qualified_name,
            replacement: replacement.clone(),
        },
        ranged,
    );

    if let Some(fix) = generate_import_edit(expr, checker, module, name, ranged)
        .or_else(|| generate_remove_and_runtime_import_edit(expr, checker, module, name))
    {
        diagnostic.set_fix(fix);
    }
}
