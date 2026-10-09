//! JSON Schema preprocessing for typify model generation.
//!
//! Used by `build.rs` at codegen time and `#[path]`-included by
//! `tests/model_codegen.rs`. The single public entry point is
//! [`rewrite`]; everything else is private composition.
//!
//! The pipeline is intentionally pure: same input always produces same
//! output, no I/O, no environment reads. Every rule matches on JSON
//! Schema *structure*, never on schema names or observed values.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";

/// Apply the schema-only preprocessing pipeline.
///
/// Pure function: same input always produces same output. Rules run in
/// this order:
///
/// 1. [`coerce_numeric_string_enums`] -- typify rejects the raw spec
///    without it.
/// 2. [`disambiguate_case_colliding_names`] -- before anything derives
///    new schema names from existing ones.
/// 3. [`lift_discriminators_to_one_of`] -- discriminated unions.
/// 4. [`unconstrained_properties_to_true`] -- keeps the lifted unions
///    named when subtypes narrow a loosely declared base property.
pub fn rewrite(raw: Value) -> Value {
    let mut value = raw;
    coerce_numeric_string_enums(&mut value);
    disambiguate_case_colliding_names(&mut value);
    lift_discriminators_to_one_of(&mut value);
    unconstrained_properties_to_true(&mut value);
    value
}

/// Recursively visit every JSON object in `value`, parents first.
fn visit_objects(value: &mut Value, f: &mut impl FnMut(&mut Map<String, Value>)) {
    match value {
        Value::Object(map) => {
            f(map);
            for v in map.values_mut() {
                visit_objects(v, f);
            }
        }
        Value::Array(items) => {
            for v in items {
                visit_objects(v, f);
            }
        }
        _ => {}
    }
}

fn schema_ref(name: &str) -> String {
    format!("{SCHEMA_REF_PREFIX}{name}")
}

/// `type: integer` (or `number`) schemas whose `enum` lists the values as
/// JSON strings get those values converted to numbers.
///
/// typify rejects the schema outright ("value does not conform to the
/// given schema"). 11.0.81 needs it for the `2.4` / `5` properties of
/// `IntegrationWifiBasicDataRateConfigurationDto` (`enum: ["1000", ...]`,
/// `example: 2000`). The declared `type` wins over the enum spelling.
/// Values that do not parse as numbers are left alone, so a genuinely
/// broken enum still fails loudly in typify.
fn coerce_numeric_string_enums(value: &mut Value) {
    visit_objects(value, &mut |map| {
        let integer = match map.get("type").and_then(Value::as_str) {
            Some("integer") => true,
            Some("number") => false,
            _ => return,
        };
        let Some(Value::Array(values)) = map.get_mut("enum") else {
            return;
        };
        for v in values {
            let Some(s) = v.as_str() else { continue };
            let parsed = if integer {
                s.parse::<i64>().ok().map(Value::from)
            } else {
                s.parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(Value::Number)
            };
            if let Some(n) = parsed {
                *v = n;
            }
        }
    });
}

/// Collision key for schema names: lowercase ASCII alphanumerics only.
///
/// typify derives type names by PascalCasing the schema name after
/// replacing non-identifier characters, so names differing only in case,
/// spacing, or punctuation collapse onto one Rust identifier. This key is
/// deliberately at least as coarse as typify's: a false positive costs a
/// harmless rename, a false negative costs duplicate type definitions.
fn collision_key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Rename schemas whose names collide after identifier sanitisation, and
/// rewrite every reference to them.
///
/// 11.0.81 has `IP Address selector` (a variant) and `IP address
/// selector` (its discriminated base); both become `IpAddressSelector`.
/// Within each colliding group, sorted by name, the first keeps its name
/// and the rest get ` 2`, ` 3`, ... appended (skipping any suffix that
/// would itself collide). Final public names are chosen in `models.rs`,
/// not here.
fn disambiguate_case_colliding_names(value: &mut Value) {
    let Some(schemas) = value
        .pointer_mut("/components/schemas")
        .and_then(Value::as_object_mut)
    else {
        return;
    };

    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in schemas.keys() {
        groups
            .entry(collision_key(name))
            .or_default()
            .push(name.clone());
    }

    let mut taken: BTreeSet<String> = groups.keys().cloned().collect();
    let mut renames: BTreeMap<String, String> = BTreeMap::new();
    for names in groups.values_mut().filter(|names| names.len() > 1) {
        names.sort();
        for name in names.iter().skip(1) {
            let mut suffix = 2;
            let new_name = loop {
                let candidate = format!("{name} {suffix}");
                if taken.insert(collision_key(&candidate)) {
                    break candidate;
                }
                suffix += 1;
            };
            renames.insert(name.clone(), new_name);
        }
    }
    if renames.is_empty() {
        return;
    }

    for (old, new) in &renames {
        if let Some(schema) = schemas.remove(old) {
            schemas.insert(new.clone(), schema);
        }
    }
    let ref_renames: BTreeMap<String, String> = renames
        .iter()
        .map(|(old, new)| (schema_ref(old), schema_ref(new)))
        .collect();
    rewrite_refs(value, &ref_renames);
}

/// Replace every schema reference found in `renames` (keyed by full
/// `#/components/schemas/...` pointer): both `$ref` values and
/// `discriminator.mapping` values, which are refs by another name.
fn rewrite_refs(value: &mut Value, renames: &BTreeMap<String, String>) {
    visit_objects(value, &mut |map| {
        if let Some(Value::String(r)) = map.get_mut("$ref") {
            if let Some(new) = renames.get(r.as_str()) {
                r.clone_from(new);
            }
        }
        if let Some(Value::Object(mapping)) = map
            .get_mut("discriminator")
            .and_then(|d| d.get_mut("mapping"))
        {
            for target in mapping.values_mut() {
                if let Value::String(r) = target {
                    if let Some(new) = renames.get(r.as_str()) {
                        r.clone_from(new);
                    }
                }
            }
        }
    });
}

/// One discriminated base schema, as found in the spec.
struct Union {
    name: String,
    property: String,
    /// `(target schema name, tags mapped to it)`, ordered by first tag
    /// (`serde_json` keeps object keys sorted).
    variants: Vec<(String, Vec<String>)>,
}

impl Union {
    /// Parse `schema` as a discriminated base, if it is one we can lift.
    ///
    /// Returns `None` when the schema has no usable `discriminator`, or
    /// when its tag property declares an `enum` that does not contain
    /// every mapping key. The spec then contradicts itself about the wire
    /// value of the tag (11.0.81: the three `Firewall policy ... named
    /// protocol` schemas map `AX_25` while the property enum says
    /// `ax.25`), and pinning either spelling could reject real payloads.
    /// Such schemas are left as plain structs.
    fn parse(name: &str, schema: &Value) -> Option<Self> {
        let discriminator = schema.get("discriminator")?;
        let property = discriminator.get("propertyName")?.as_str()?;
        let mut variants: Vec<(String, Vec<String>)> = Vec::new();
        for (tag, target) in discriminator.get("mapping")?.as_object()? {
            let target = target.as_str()?.strip_prefix(SCHEMA_REF_PREFIX)?;
            match variants.iter_mut().find(|(t, _)| t == target) {
                Some((_, tags)) => tags.push(tag.clone()),
                None => variants.push((target.to_string(), vec![tag.clone()])),
            }
        }
        if variants.is_empty() {
            return None;
        }
        if let Some(Value::Array(allowed)) = schema
            .get("properties")
            .and_then(|p| p.get(property))
            .and_then(|p| p.get("enum"))
        {
            let covered = variants
                .iter()
                .flat_map(|(_, tags)| tags)
                .all(|tag| allowed.iter().any(|a| a.as_str() == Some(tag)));
            if !covered {
                return None;
            }
        }
        Some(Self {
            name: name.to_string(),
            property: property.to_string(),
            variants,
        })
    }
}

/// Turn every `discriminator`-without-`oneOf` base into a real `oneOf`
/// whose variants typify can tell apart.
///
/// The spec (springdoc-generated) models polymorphism Java-style: a base
/// schema `B` carries `discriminator: {propertyName, mapping}` and its own
/// properties, and each mapping target is `allOf: [{$ref: B}, {extra
/// props}]`. There is no `oneOf` anywhere, so typify ignores the
/// discriminator and emits `B` as a struct with only the shared fields
/// (the tag as a bare `String`): variant fields are unreachable from any
/// field typed `B`. 77 schemas in 11.0.81 have this shape, including
/// responses (`Network details`, `Client details`, ...).
///
/// For each such `B` with tag property `P` (see [`Union::parse`] for the
/// ones skipped):
///
/// 1. `B`'s own body, minus the discriminator, moves to a new schema
///    `"<B> base"`.
/// 2. Every `allOf` member referencing `B`, anywhere in the spec, is
///    repointed at `"<B> base"`. This is inheritance, not a use of the
///    union, and repointing breaks the `B` → target → `B` cycle.
/// 3. Each mapping target gets `P` pinned to its tag(s): a required
///    string `enum` of every tag mapped to it (one tag, except for
///    catch-all targets). Targets that never `allOf` `B` (11.0.81 has
///    12, e.g. `System defined entity metadata`) get the same pin.
/// 4. `B` becomes `oneOf` over `$ref`s to its mapping targets.
///
/// typify emits the result as an `#[serde(untagged)]` enum with one
/// newtype variant per named target. The pins make the variants mutually
/// exclusive, so decoding is unambiguous and encoding writes the tag.
/// (typify only emits `#[serde(tag = ...)]` when every variant is an
/// inline object, which would cost the named variant types.)
///
/// A target shared by several unions with the same pin (the `... entity
/// metadata` family) is pinned once. A target the unions pin
/// *differently* cannot be pinned in place; each union instead wraps it
/// as `allOf: [{$ref: target}, {pin}]`.
fn lift_discriminators_to_one_of(value: &mut Value) {
    let Some(schemas) = value
        .pointer_mut("/components/schemas")
        .and_then(Value::as_object_mut)
    else {
        return;
    };

    let unions: Vec<Union> = schemas
        .iter()
        .filter_map(|(name, schema)| Union::parse(name, schema))
        .collect();
    if unions.is_empty() {
        return;
    }

    // Shapes this rule would silently get wrong; none occur in 11.0.81.
    // Fail the build loudly instead (see UPGRADING.md, "When codegen
    // fails").
    let union_names: BTreeSet<&str> = unions.iter().map(|u| u.name.as_str()).collect();
    for union in &unions {
        let base_name = format!("{} base", union.name);
        assert!(
            !schemas.contains_key(&base_name),
            "lift_discriminators_to_one_of: {base_name:?} already exists in the spec"
        );
        if let Some((target, _)) = union
            .variants
            .iter()
            .find(|(target, _)| union_names.contains(target.as_str()))
        {
            panic!(
                "lift_discriminators_to_one_of: {:?} maps to {target:?}, itself a \
                 discriminated union; nested unions are not supported",
                union.name
            );
        }
    }

    // Every distinct pin each target receives across all unions.
    let mut pins: BTreeMap<&str, BTreeSet<(&str, &[String])>> = BTreeMap::new();
    for union in &unions {
        for (target, tags) in &union.variants {
            pins.entry(target)
                .or_default()
                .insert((union.property.as_str(), tags.as_slice()));
        }
    }
    let pin_in_place = |target: &str| pins.get(target).is_some_and(|p| p.len() == 1);

    // 1. Move each base body to "<B> base"; 4. replace B with a oneOf.
    let mut base_refs: BTreeMap<String, String> = BTreeMap::new();
    for union in &unions {
        let base_name = format!("{} base", union.name);
        let Some(Value::Object(mut body)) = schemas.remove(&union.name) else {
            continue;
        };
        body.remove("discriminator");
        let description = body.get("description").cloned();
        schemas.insert(base_name.clone(), Value::Object(body));
        base_refs.insert(schema_ref(&union.name), schema_ref(&base_name));

        let one_of: Vec<Value> = union
            .variants
            .iter()
            .map(|(target, tags)| {
                let target_ref = json!({ "$ref": schema_ref(target) });
                if pin_in_place(target) {
                    target_ref
                } else {
                    json!({ "allOf": [target_ref, tag_pin(&union.property, tags)] })
                }
            })
            .collect();
        let mut lifted = Map::new();
        if let Some(description) = description {
            lifted.insert("description".to_string(), description);
        }
        lifted.insert("oneOf".to_string(), Value::Array(one_of));
        schemas.insert(union.name.clone(), Value::Object(lifted));
    }

    // 2. Repoint inheritance (`allOf` members) from each base to its body.
    for schema in schemas.values_mut() {
        visit_objects(schema, &mut |map| {
            let Some(Value::Array(members)) = map.get_mut("allOf") else {
                return;
            };
            for member in members {
                if let Some(Value::String(r)) = member.get_mut("$ref") {
                    if let Some(new) = base_refs.get(r.as_str()) {
                        r.clone_from(new);
                    }
                }
            }
        });
    }

    // 3. Pin the tag inside each target with a single, unambiguous pin.
    for (target, target_pins) in &pins {
        let [(property, tags)] = *target_pins.iter().copied().collect::<Vec<_>>() else {
            continue;
        };
        if let Some(Value::Object(body)) = schemas.get_mut(*target) {
            pin_tag(body, property, tags);
        }
    }
}

/// The schema of a tag property restricted to `tags`.
fn tag_schema(tags: &[String]) -> Value {
    json!({ "type": "string", "enum": tags })
}

/// An inline object schema requiring `property` to be one of `tags`.
fn tag_pin(property: &str, tags: &[String]) -> Value {
    json!({
        "type": "object",
        "properties": { property: tag_schema(tags) },
        "required": [property],
    })
}

/// Pin `property` to `tags` in a target schema body, which is either a
/// plain object schema or an `allOf` composition.
fn pin_tag(body: &mut Map<String, Value>, property: &str, tags: &[String]) {
    if let Some(Value::Array(members)) = body.get_mut("allOf") {
        members.push(tag_pin(property, tags));
        return;
    }
    if let Value::Object(properties) = body
        .entry("properties")
        .or_insert_with(|| Value::Object(Map::new()))
    {
        properties.insert(property.to_string(), tag_schema(tags));
    }
    if let Value::Array(required) = body
        .entry("required")
        .or_insert_with(|| Value::Array(Vec::new()))
    {
        if !required.iter().any(|r| r.as_str() == Some(property)) {
            required.push(Value::String(property.to_string()));
        }
    }
}

/// Keywords that annotate a schema without constraining it.
const ANNOTATION_KEYWORDS: &[&str] = &[
    "deprecated",
    "description",
    "example",
    "examples",
    "readOnly",
    "title",
    "writeOnly",
];

/// True for an object schema made only of annotations: it accepts any
/// value, exactly like `true`.
fn is_annotation_only(schema: &Map<String, Value>) -> bool {
    schema
        .keys()
        .all(|k| ANNOTATION_KEYWORDS.contains(&k.as_str()))
}

/// Replace unconstrained property schemas with the boolean schema `true`.
///
/// "Unconstrained" means annotation-only (`{}`, `{description: ...}`) or a
/// bare `$ref` to an annotation-only component (11.0.81: `Client access
/// overview`, `Access point feature overview`). Both accept any value,
/// as `true` does, so the generated field type is unchanged
/// (`serde_json::Value`).
///
/// The difference matters inside `allOf`. Abstract bases declare a
/// property loosely (`ACL rule base` has `sourceFilter: {description}`,
/// `Client overview base` has `access: {$ref: Client access overview}`)
/// and each subtype narrows it to a lifted union. typify merges
/// `true` ∧ `$ref` to the plain `$ref`, but resolves and inlines the
/// union for the other spellings, emitting anonymous `...Variant0`
/// duplicates of every variant instead of the named union type.
fn unconstrained_properties_to_true(value: &mut Value) {
    let Some(schemas) = value
        .pointer("/components/schemas")
        .and_then(Value::as_object)
    else {
        return;
    };
    let unconstrained_refs: BTreeSet<String> = schemas
        .iter()
        .filter(|(_, schema)| schema.as_object().is_some_and(is_annotation_only))
        .map(|(name, _)| schema_ref(name))
        .collect();

    visit_objects(value, &mut |map| {
        let Some(Value::Object(properties)) = map.get_mut("properties") else {
            return;
        };
        for property in properties.values_mut() {
            let Value::Object(schema) = property else {
                continue;
            };
            let refs_unconstrained = schema.len() == 1
                && schema
                    .get("$ref")
                    .and_then(Value::as_str)
                    .is_some_and(|r| unconstrained_refs.contains(r));
            if is_annotation_only(schema) || refs_unconstrained {
                *property = Value::Bool(true);
            }
        }
    });
}
