#![forbid(unsafe_code)]

//! Smoke tests for the generated-model seam, and unit tests for the
//! build-time spec rewrite.
//!
//! Seam tests guard three things:
//!
//! 1. **Surface fingerprint.** `_seam_signatures` names every type
//!    `models.rs` re-exports. A spec bump that renames or removes one
//!    fails compilation here before it fails in a wrapper module.
//! 2. **Derive fingerprint.** `_assert_derives` asserts the trait set
//!    every re-exported model provides.
//! 3. **Union round-trips.** One request union (serialise) and one
//!    response union (deserialise) prove the discriminator rewrite in
//!    `build_support/spec_rewrite.rs` produced tag-carrying enums.
//!
//! Rewrite tests drive `spec_rewrite::rewrite` (`#[path]`-included, the
//! same file `build.rs` uses) with minimal synthetic specs, one rule at a
//! time, plus a whole-spec idempotence check against the pinned spec.

use ferro_network::models::{
    ApplicationInfo, ClientActionRequest, GatewayManagedNetworkDetails,
    GuestAccessAuthorizationRequest, GuestAccessAuthorizationRequestAction,
    GuestAccessUnauthorizationRequest, GuestAccessUnauthorizationRequestAction, NetworkDetails,
    SwitchManagedNetworkDetails, UnmanagedNetworkDetails,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[path = "../build_support/spec_rewrite.rs"]
mod spec_rewrite;

// --- seam ---------------------------------------------------------------

#[test]
fn application_info_round_trips() {
    let info: ApplicationInfo =
        serde_json::from_str(include_str!("fixtures/info_ok.json")).expect("fixture parses");
    assert_eq!(info.application_version, "10.0.160");

    let encoded = serde_json::to_string(&info).expect("serializes");
    let reparsed: ApplicationInfo = serde_json::from_str(&encoded).expect("re-parses");
    assert_eq!(reparsed, info);
}

#[test]
fn request_union_serialises_its_tag() {
    let authorize = ClientActionRequest::AuthorizationRequest(GuestAccessAuthorizationRequest {
        action: GuestAccessAuthorizationRequestAction::AuthorizeGuestAccess,
        time_limit_minutes: std::num::NonZeroU64::new(60),
        data_usage_limit_m_bytes: None,
        rx_rate_limit_kbps: None,
        tx_rate_limit_kbps: None,
    });
    assert_eq!(
        serde_json::to_value(&authorize).expect("serializes"),
        json!({ "action": "AUTHORIZE_GUEST_ACCESS", "timeLimitMinutes": 60 })
    );

    let unauthorize =
        ClientActionRequest::UnauthorizationRequest(GuestAccessUnauthorizationRequest {
            action: GuestAccessUnauthorizationRequestAction::UnauthorizeGuestAccess,
        });
    assert_eq!(
        serde_json::to_value(&unauthorize).expect("serializes"),
        json!({ "action": "UNAUTHORIZE_GUEST_ACCESS" })
    );
}

#[test]
fn response_union_decodes_by_tag() {
    let fixture = include_str!("fixtures/network_details_unmanaged.json");
    let details: NetworkDetails = serde_json::from_str(fixture).expect("fixture decodes");
    let NetworkDetails::UnmanagedNetworkDetails(unmanaged) = &details else {
        panic!("expected the UNMANAGED variant, got {details:?}");
    };
    assert_eq!(unmanaged.vlan_id.get(), 40);
    assert_eq!(*unmanaged.name, "Lab");

    // The tag is part of the payload, so re-encoding reproduces the input.
    let original: Value = serde_json::from_str(fixture).expect("fixture is JSON");
    assert_eq!(
        serde_json::to_value(&details).expect("serializes"),
        original
    );
}

#[test]
fn response_union_rejects_unknown_tag() {
    let mut fixture: Value =
        serde_json::from_str(include_str!("fixtures/network_details_unmanaged.json"))
            .expect("fixture is JSON");
    fixture["management"] = json!("BOGUS");
    assert!(serde_json::from_value::<NetworkDetails>(fixture).is_err());
}

// Compile-only: every re-exported model satisfies the trait set wrappers
// and consumers rely on. If typify stops emitting one of these derives,
// this stops compiling.
const fn _assert_derives() {
    const fn assert_model<
        T: Serialize + for<'de> Deserialize<'de> + Clone + std::fmt::Debug + PartialEq,
    >() {
    }
    assert_model::<ApplicationInfo>();
    assert_model::<ClientActionRequest>();
    assert_model::<GatewayManagedNetworkDetails>();
    assert_model::<GuestAccessAuthorizationRequest>();
    assert_model::<GuestAccessAuthorizationRequestAction>();
    assert_model::<GuestAccessUnauthorizationRequest>();
    assert_model::<GuestAccessUnauthorizationRequestAction>();
    assert_model::<NetworkDetails>();
    assert_model::<SwitchManagedNetworkDetails>();
    assert_model::<UnmanagedNetworkDetails>();
}

// Compile-only fingerprint of the seam: name every re-exported type in a
// signature so a rename in `models.rs` or in the typify output it pulls
// from fails here before reaching wrapper code.
#[expect(
    clippy::too_many_arguments,
    reason = "one parameter per re-exported type is the point"
)]
fn _seam_signatures(
    _info: ApplicationInfo,
    _client_action: ClientActionRequest,
    _gateway: GatewayManagedNetworkDetails,
    _authorize: GuestAccessAuthorizationRequest,
    _authorize_action: GuestAccessAuthorizationRequestAction,
    _unauthorize: GuestAccessUnauthorizationRequest,
    _unauthorize_action: GuestAccessUnauthorizationRequestAction,
    _network: NetworkDetails,
    _switch: SwitchManagedNetworkDetails,
    _unmanaged: UnmanagedNetworkDetails,
) {
}

// --- spec_rewrite -------------------------------------------------------

/// Wrap `schemas` in the minimal spec shape `rewrite` walks.
fn spec(schemas: Value) -> Value {
    let mut spec = json!({ "components": {} });
    spec["components"]["schemas"] = schemas;
    spec
}

fn rewritten_schemas(schemas: Value) -> Value {
    spec_rewrite::rewrite(spec(schemas))["components"]["schemas"].take()
}

#[test]
fn rewrite_coerces_numeric_string_enums() {
    let out = rewritten_schemas(json!({
        "Rates": {
            "type": "object",
            "properties": {
                "int": { "type": "integer", "enum": ["1000", "2000"] },
                "num": { "type": "number", "enum": ["1.5"] },
                "str": { "type": "string", "enum": ["1000"] },
                "junk": { "type": "integer", "enum": ["fast"] },
            },
        },
    }));
    let props = &out["Rates"]["properties"];
    assert_eq!(props["int"]["enum"], json!([1000, 2000]));
    assert_eq!(props["num"]["enum"], json!([1.5]));
    assert_eq!(
        props["str"]["enum"],
        json!(["1000"]),
        "strings stay strings"
    );
    assert_eq!(
        props["junk"]["enum"],
        json!(["fast"]),
        "unparseable values are left for typify to reject"
    );
}

#[test]
fn rewrite_disambiguates_colliding_names_and_their_refs() {
    let out = rewritten_schemas(json!({
        "IP Address selector": { "type": "object" },
        "IP address selector": {
            "discriminator": {
                "propertyName": "type",
                "mapping": { "IP_ADDRESS": "#/components/schemas/IP Address selector" },
            },
            "properties": { "type": { "type": "string" } },
        },
        "Holder": {
            "type": "object",
            "properties": { "sel": { "$ref": "#/components/schemas/IP address selector" } },
        },
    }));
    let names: Vec<&str> = out
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert!(names.contains(&"IP Address selector"));
    assert!(names.contains(&"IP address selector 2"));
    assert!(!names.contains(&"IP address selector"));
    assert_eq!(
        out["Holder"]["properties"]["sel"]["$ref"],
        "#/components/schemas/IP address selector 2"
    );
    // The renamed base was then lifted, and its oneOf still reaches the
    // (unrenamed) variant.
    assert_eq!(
        out["IP address selector 2"]["oneOf"],
        json!([{ "$ref": "#/components/schemas/IP Address selector" }])
    );
}

/// `Pet` discriminated on `kind`: `Dog` inherits and adds a field, `Cat`
/// inherits, and `Robot` is mapped without inheriting.
fn pet_union() -> Value {
    json!({
        "Pet": {
            "type": "object",
            "description": "A pet.",
            "discriminator": {
                "propertyName": "kind",
                "mapping": {
                    "DOG": "#/components/schemas/Dog",
                    "CAT": "#/components/schemas/Cat",
                    "ROBOT": "#/components/schemas/Robot",
                },
            },
            "properties": { "kind": { "type": "string" }, "name": { "type": "string" } },
            "required": ["kind", "name"],
        },
        "Dog": {
            "allOf": [
                { "$ref": "#/components/schemas/Pet" },
                { "type": "object", "properties": { "barks": { "type": "boolean" } } },
            ],
        },
        "Cat": { "allOf": [{ "$ref": "#/components/schemas/Pet" }] },
        "Robot": { "type": "object", "properties": { "kind": { "type": "string" } } },
        "Owner": {
            "type": "object",
            "properties": { "pet": { "$ref": "#/components/schemas/Pet" } },
        },
    })
}

#[test]
fn rewrite_lifts_discriminator_into_one_of() {
    let out = rewritten_schemas(pet_union());

    assert_eq!(
        out["Pet"],
        json!({
            "description": "A pet.",
            "oneOf": [
                { "$ref": "#/components/schemas/Cat" },
                { "$ref": "#/components/schemas/Dog" },
                { "$ref": "#/components/schemas/Robot" },
            ],
        })
    );

    // The base body moved, minus its discriminator.
    assert_eq!(
        out["Pet base"]["properties"]["name"],
        json!({ "type": "string" })
    );
    assert!(out["Pet base"].get("discriminator").is_none());

    // Inheritance points at the body; uses of the union do not move.
    assert_eq!(
        out["Dog"]["allOf"][0]["$ref"],
        "#/components/schemas/Pet base"
    );
    assert_eq!(
        out["Owner"]["properties"]["pet"]["$ref"],
        "#/components/schemas/Pet"
    );

    // Each target is pinned to its tag: appended to an allOf, or written
    // straight into a plain object (which never inherited the base).
    assert_eq!(
        out["Dog"]["allOf"][2],
        json!({
            "type": "object",
            "properties": { "kind": { "type": "string", "enum": ["DOG"] } },
            "required": ["kind"],
        })
    );
    assert_eq!(
        out["Robot"]["properties"]["kind"],
        json!({ "type": "string", "enum": ["ROBOT"] })
    );
    assert_eq!(out["Robot"]["required"], json!(["kind"]));
}

#[test]
fn rewrite_collapses_tags_sharing_one_target() {
    let out = rewritten_schemas(json!({
        "Proto": {
            "discriminator": {
                "propertyName": "name",
                "mapping": {
                    "ICMP": "#/components/schemas/Icmp",
                    "AH": "#/components/schemas/Other",
                    "GRE": "#/components/schemas/Other",
                },
            },
            "properties": { "name": { "type": "string" } },
        },
        "Icmp": { "allOf": [{ "$ref": "#/components/schemas/Proto" }] },
        "Other": { "allOf": [{ "$ref": "#/components/schemas/Proto" }] },
    }));
    assert_eq!(
        out["Proto"]["oneOf"],
        json!([
            { "$ref": "#/components/schemas/Other" },
            { "$ref": "#/components/schemas/Icmp" },
        ])
    );
    assert_eq!(
        out["Other"]["allOf"][1]["properties"]["name"]["enum"],
        json!(["AH", "GRE"])
    );
}

#[test]
fn rewrite_skips_unions_whose_tag_enum_contradicts_the_mapping() {
    let schemas = json!({
        "Proto": {
            "discriminator": {
                "propertyName": "name",
                "mapping": { "AX_25": "#/components/schemas/Ax25" },
            },
            "properties": { "name": { "type": "string", "enum": ["ax.25"] } },
        },
        "Ax25": { "allOf": [{ "$ref": "#/components/schemas/Proto" }] },
    });
    assert_eq!(rewritten_schemas(schemas.clone()), schemas);
}

#[test]
fn rewrite_pins_shared_targets_once_or_wraps_on_conflict() {
    let out = rewritten_schemas(json!({
        "A": {
            "discriminator": {
                "propertyName": "origin",
                "mapping": {
                    "USER": "#/components/schemas/User",
                    "X": "#/components/schemas/Odd",
                },
            },
            "properties": { "origin": { "type": "string" } },
        },
        "B": {
            "discriminator": {
                "propertyName": "origin",
                "mapping": {
                    "USER": "#/components/schemas/User",
                    "Y": "#/components/schemas/Odd",
                },
            },
            "properties": { "origin": { "type": "string" } },
        },
        "User": {
            "allOf": [{ "$ref": "#/components/schemas/A" }, { "$ref": "#/components/schemas/B" }],
        },
        "Odd": { "type": "object", "properties": { "origin": { "type": "string" } } },
    }));

    // Same pin from both unions: applied once, in place.
    assert_eq!(out["User"]["allOf"].as_array().unwrap().len(), 3);
    assert_eq!(
        out["A"]["oneOf"][0],
        json!({ "$ref": "#/components/schemas/User" })
    );

    // Different pins: the target stays untouched and each union wraps it.
    assert_eq!(
        out["Odd"]["properties"]["origin"],
        json!({ "type": "string" })
    );
    assert_eq!(
        out["B"]["oneOf"][1]["allOf"][1]["properties"]["origin"]["enum"],
        json!(["Y"])
    );
}

#[test]
#[should_panic(expected = "nested unions are not supported")]
fn rewrite_refuses_nested_unions() {
    let mut schemas = pet_union();
    schemas["Robot"] = json!({
        "discriminator": {
            "propertyName": "model",
            "mapping": { "T800": "#/components/schemas/Cat" },
        },
        "properties": { "model": { "type": "string" } },
    });
    rewritten_schemas(schemas);
}

#[test]
#[should_panic(expected = "\"Pet base\" already exists")]
fn rewrite_refuses_to_overwrite_a_base_name() {
    let mut schemas = pet_union();
    schemas["Pet base"] = json!({ "type": "object" });
    rewritten_schemas(schemas);
}

#[test]
fn rewrite_turns_unconstrained_properties_into_true() {
    let out = rewritten_schemas(json!({
        "Anything": { "example": { "type": "DEFAULT" } },
        "Thing": { "type": "object" },
        "Holder": {
            "type": "object",
            "properties": {
                "empty": {},
                "described": { "description": "Any value." },
                "to_any": { "$ref": "#/components/schemas/Anything" },
                "to_thing": { "$ref": "#/components/schemas/Thing" },
                "typed": { "type": "string", "description": "A string." },
            },
        },
    }));
    let props = &out["Holder"]["properties"];
    assert_eq!(props["empty"], json!(true));
    assert_eq!(props["described"], json!(true));
    assert_eq!(props["to_any"], json!(true));
    assert_eq!(
        props["to_thing"],
        json!({ "$ref": "#/components/schemas/Thing" })
    );
    assert_eq!(props["typed"]["type"], "string");
}

#[test]
fn rewrite_is_idempotent_on_the_pinned_spec() {
    let raw: Value = serde_json::from_str(
        &std::fs::read_to_string(pinned_spec_path()).expect("spec submodule is checked out"),
    )
    .expect("spec is JSON");
    let once = spec_rewrite::rewrite(raw);
    let twice = spec_rewrite::rewrite(once.clone());
    assert!(once == twice, "a second rewrite pass changed the spec");
}

/// The spec `build.rs` reads, located through its `SPEC_VERSION` line so
/// this test never pins a version of its own.
fn pinned_spec_path() -> std::path::PathBuf {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let build_rs = std::fs::read_to_string(crate_dir.join("build.rs")).expect("build.rs exists");
    let version = build_rs
        .lines()
        .find_map(|line| line.strip_prefix("const SPEC_VERSION: &str = \""))
        .and_then(|rest| rest.strip_suffix("\";"))
        .expect("build.rs declares SPEC_VERSION");
    crate_dir.join(format!(
        "../../third_party/unifi-apis/unifi-network/{version}.json"
    ))
}
