use std::collections::HashSet;

use anyhow::Result;
use baml_types::BamlValue;
use indexmap::IndexSet;
use internal_baml_core::ir::{
    repr::IntermediateRepr, ClassWalker, EnumWalker, FieldType, IRHelper,
};
use internal_baml_jinja::types::{Class, Enum, Name, OutputFormatContent};

use crate::{
    runtime_context::{RuntimeClassOverride, RuntimeEnumOverride},
    RuntimeContext,
};

pub fn render_output_format(
    ir: &IntermediateRepr,
    ctx: &RuntimeContext,
    output: &FieldType,
) -> Result<OutputFormatContent> {
    let (enums, classes, recursive_classes) = relevant_data_models(ir, output, ctx)?;

    Ok(OutputFormatContent::target(output.clone())
        .enums(enums)
        .classes(classes)
        .recursive_classes(recursive_classes)
        .build())
}

enum OverridableValue<T> {
    Unset,
    SetEmpty,
    Set(T),
}

impl<T> OverridableValue<T> {
    fn value(self) -> Option<T> {
        match self {
            OverridableValue::Set(v) => Some(v),
            _ => None,
        }
    }
}

impl OverridableValue<bool> {
    fn from(v: Option<&BamlValue>) -> Self {
        match v {
            Some(v) => match v.as_bool() {
                Some(v) => OverridableValue::Set(v),
                None => OverridableValue::SetEmpty,
            },
            None => OverridableValue::Unset,
        }
    }
}

impl OverridableValue<String> {
    fn from(v: Option<&BamlValue>) -> Self {
        match v {
            Some(v) => match v.as_str() {
                Some(v) => OverridableValue::Set(v.to_string()),
                None => OverridableValue::SetEmpty,
            },
            None => OverridableValue::Unset,
        }
    }
}

fn find_new_class_field(
    class_name: &str,
    field_name: &str,
    class_walker: &Result<ClassWalker<'_>>,
    overrides: &RuntimeClassOverride,
    _ctx: &RuntimeContext,
) -> Result<(Name, FieldType, Option<String>)> {
    let Some(field_overrides) = overrides.new_fields.get(field_name) else {
        anyhow::bail!("Class {} does not have a field: {}", class_name, field_name);
    };

    // Ensure the original field does not exist
    if let Ok(class_walker) = class_walker {
        if class_walker.find_field(field_name).is_some() {
            anyhow::bail!(
                "Class {} already has a pre-defined field: {}",
                class_name,
                field_name
            );
        }
    }

    let alias = OverridableValue::<String>::from(field_overrides.1.alias.as_ref());
    let desc = OverridableValue::<String>::from(field_overrides.1.meta.get("description"));

    let name = Name::new_with_alias(field_name.to_string(), alias.value());
    let desc = desc.value();

    Ok((name, field_overrides.0.clone(), desc))
}

fn find_existing_class_field(
    class_name: &str,
    field_name: &str,
    class_walker: &Result<ClassWalker<'_>>,
    overrides: &Option<&RuntimeClassOverride>,
    ctx: &RuntimeContext,
) -> Result<(Name, FieldType, Option<String>)> {
    let Ok(class_walker) = class_walker else {
        anyhow::bail!("Class {} does not exist", class_name);
    };

    let Some(field_walker) = class_walker.find_field(field_name) else {
        anyhow::bail!("Class {} does not have a field: {}", class_name, field_name);
    };

    let field_overrides = overrides.map(|o| o.update_fields.get(field_name)).flatten();

    let mut alias = OverridableValue::Unset;
    let mut desc = OverridableValue::Unset;

    if let Some(attrs) = field_overrides {
        alias = OverridableValue::<String>::from(attrs.alias.as_ref());
        desc = OverridableValue::<String>::from(attrs.meta.get("description"));
    }

    let eval_ctx = ctx.eval_ctx(false);

    if matches!(alias, OverridableValue::Unset) {
        if let Some(_alias) = field_walker.alias(&eval_ctx)? {
            alias = OverridableValue::Set(_alias);
        }
    }

    if matches!(desc, OverridableValue::Unset) {
        if let Some(_desc) = field_walker.description(&eval_ctx)? {
            desc = OverridableValue::Set(_desc);
        }
    }

    let name = Name::new_with_alias(field_name.to_string(), alias.value());
    let desc = desc.value();
    let r#type = field_walker.r#type();
    Ok((name, r#type.clone(), desc))
}

fn find_enum_value(
    enum_name: &str,
    value_name: &str,
    enum_walker: &Result<EnumWalker<'_>>,
    overrides: &Option<&RuntimeEnumOverride>,
    ctx: &RuntimeContext,
) -> Result<Option<(Name, Option<String>)>> {
    if enum_walker.is_err() && overrides.is_none() {
        anyhow::bail!("Enum {} does not exist", enum_name);
    }

    let value_walker = match enum_walker {
        Ok(e) => e.find_value(value_name),
        Err(_) => None,
    };

    let value_overrides = overrides.map(|o| o.values.get(value_name)).flatten();

    if value_overrides.is_none() && value_walker.is_none() {
        anyhow::bail!("Enum {} does not have a value: {}", enum_name, value_name);
    }

    let mut skip = OverridableValue::Unset;
    let mut alias = OverridableValue::Unset;
    let mut desc = OverridableValue::Unset;
    if let Some(attrs) = value_overrides {
        match attrs.skip {
            Some(true) => return Ok(None),
            Some(false) => skip = OverridableValue::Set(false),
            None => {}
        }

        alias = OverridableValue::<String>::from(attrs.alias.as_ref());
        desc = OverridableValue::<String>::from(attrs.meta.get("description"));
    }

    let eval_ctx = ctx.eval_ctx(false);
    if let Some(value) = value_walker {
        if value.skip(&eval_ctx)? && !matches!(skip, OverridableValue::Set(false)) {
            return Ok(None);
        }
        if matches!(alias, OverridableValue::Unset) {
            if let Some(_alias) = value.alias(&eval_ctx)? {
                alias = OverridableValue::Set(_alias);
            }
        }

        if matches!(desc, OverridableValue::Unset) {
            if let Some(_desc) = value.description(&eval_ctx)? {
                desc = OverridableValue::Set(_desc);
            }
        }
    }

    let name = Name::new_with_alias(value_name.to_string(), alias.value());
    let desc = desc.value();

    Ok(Some((name, desc)))
}

// TODO: This function is "almost" a duplicate of `relevant_dat_models` at
// baml-lib/jsonish/src/tests/mod.rs
//
// Should be refactored.
fn relevant_data_models<'a>(
    ir: &'a IntermediateRepr,
    output: &'a FieldType,
    ctx: &RuntimeContext,
) -> Result<(Vec<Enum>, Vec<Class>, IndexSet<String>)> {
    let mut checked_types = HashSet::new();
    let mut enums = Vec::new();
    let mut classes = Vec::new();
    let mut recursive_classes = IndexSet::new();
    let mut start: Vec<baml_types::FieldType> = vec![output.clone()];

    let eval_ctx = ctx.eval_ctx(false);

    while let Some(output) = start.pop() {
        match ir.distribute_constraints(&output) {
            (FieldType::Enum(enm), constraints) => {
                if checked_types.insert(output.to_string()) {
                    let overrides = ctx.enum_overrides.get(enm);
                    let walker = ir.find_enum(enm);

                    let real_values = walker
                        .as_ref()
                        .map(|e| e.walk_values().map(|v| v.name().to_string()))
                        .ok();
                    let override_values = overrides
                        .map(|o| o.values.keys().cloned())
                        .into_iter()
                        .flatten();
                    let values = real_values
                        .into_iter()
                        .flatten()
                        .chain(override_values)
                        .collect::<IndexSet<_>>()
                        .into_iter()
                        .map(|value| {
                            let meta = find_enum_value(enm, &value, &walker, &overrides, ctx)?;
                            Ok(meta)
                        })
                        .filter_map(|v| v.transpose())
                        .collect::<Result<Vec<_>>>()?;

                    let mut alias =
                        OverridableValue::<String>::from(overrides.and_then(|o| o.alias.as_ref()));

                    if matches!(alias, OverridableValue::Unset) {
                        if let Ok(walker) = walker {
                            if let Some(a) = walker.alias(&eval_ctx)? {
                                alias = OverridableValue::Set(a);
                            }
                        }
                    }

                    enums.push(Enum {
                        name: Name::new_with_alias(enm.to_string(), alias.value()),
                        values,
                        constraints,
                    });
                }
            }
            (FieldType::List(inner), _) | (FieldType::Optional(inner), _) => {
                if !checked_types.contains(&inner.to_string()) {
                    start.push(inner.as_ref().clone());
                }
            }
            (FieldType::Map(k, v), _) => {
                if checked_types.insert(output.to_string()) {
                    if !checked_types.contains(&k.to_string()) {
                        start.push(k.as_ref().clone());
                    }
                    if !checked_types.contains(&v.to_string()) {
                        start.push(v.as_ref().clone());
                    }
                }
            }
            (FieldType::Tuple(options), _) | (FieldType::Union(options), _) => {
                if checked_types.insert(output.to_string()) {
                    for inner in options {
                        if !checked_types.contains(&inner.to_string()) {
                            start.push(inner.clone());
                        }
                    }
                }
            }
            (FieldType::Class(cls), constraints) => {
                if checked_types.insert(output.to_string()) {
                    let overrides = ctx.class_override.get(cls);
                    let walker = ir.find_class(cls);

                    let real_fields = walker
                        .as_ref()
                        .map(|e| e.walk_fields().map(|v| v.name().to_string()))
                        .ok();
                    let override_fields = overrides
                        .map(|o| o.update_fields.keys().cloned())
                        .into_iter()
                        .flatten();

                    let fields = real_fields
                        .into_iter()
                        .flatten()
                        .chain(override_fields)
                        .collect::<IndexSet<_>>()
                        .into_iter()
                        .map(|field| {
                            let meta =
                                find_existing_class_field(cls, &field, &walker, &overrides, ctx)?;
                            Ok(meta)
                        });

                    let new_fields = overrides
                        .map(|o| {
                            o.new_fields
                                .keys()
                                .map(|k| find_new_class_field(cls, k, &walker, o, ctx))
                        })
                        .into_iter()
                        .flatten();

                    let mut alias =
                        OverridableValue::<String>::from(overrides.and_then(|o| o.alias.as_ref()));

                    if matches!(alias, OverridableValue::Unset) {
                        if let Ok(walker) = walker {
                            if let Some(a) = walker.alias(&eval_ctx)? {
                                alias = OverridableValue::Set(a);
                            }
                        }
                    }

                    let fields = fields.chain(new_fields).collect::<Result<Vec<_>>>()?;

                    for (_, t, _) in fields.iter().as_ref() {
                        if !checked_types.contains(&t.to_string()) {
                            start.push(t.clone());
                        }
                    }

                    // TODO: O(n) algorithm. Maybe a Merge-Find Set can optimize
                    // this to O(log n) or something like that
                    // (maybe, IDK though ¯\_(ツ)_/¯)
                    //
                    // Also there's a lot of cloning in this process of going
                    // from Parser DB to IR to Jinja Output Format, not only
                    // with recursive classes but also the rest of models.
                    // There's room for optimization here.
                    //
                    // Also take a look at the TODO on top of this function.
                    for cycle in ir.finite_recursive_cycles() {
                        if cycle.contains(cls) {
                            recursive_classes.extend(cycle.iter().map(ToOwned::to_owned));
                        }
                    }

                    classes.push(Class {
                        name: Name::new_with_alias(cls.to_string(), alias.value()),
                        fields,
                        constraints,
                    });
                } else {
                    recursive_classes.insert(cls.to_owned());
                }
            }
            (FieldType::Literal(_), _) => {}
            (FieldType::Primitive(_), _) => {}
            (FieldType::Constrained { .. }, _) => {
                unreachable!("It is guaranteed that a call to distribute_constraints will not return FieldType::Constrained")
            }
        }
    }

    Ok((enums, classes, recursive_classes))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::BamlRuntime;

    #[test]
    fn skipped_variants_are_not_rendered() {
        let files = vec![(
            "test-file.baml",
            r#"
          enum Foo {
            Bar
            Baz @skip
          }"#,
        )]
        .into_iter()
        .collect();
        let env_vars: HashMap<&str, &str> = HashMap::new();
        let baml_runtime = BamlRuntime::from_file_content(".", &files, env_vars).unwrap();
        let ctx_manager = baml_runtime.create_ctx_manager(BamlValue::Null, None);
        let ctx: RuntimeContext = ctx_manager.create_ctx(None, None).unwrap();

        let field_type = FieldType::Enum("Foo".to_string());
        let render_output =
            render_output_format(baml_runtime.inner.ir.as_ref(), &ctx, &field_type).unwrap();

        let foo_enum = render_output.find_enum("Foo").unwrap();
        assert_eq!(foo_enum.values[0].0.real_name(), "Bar".to_string());
        assert_eq!(foo_enum.values.len(), 1);
    }
}
