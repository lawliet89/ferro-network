//! Public model types: the seam between typify-generated code and the
//! rest of the crate.
//!
//! Every generated type that crosses a public signature is re-exported
//! here, renamed where the spec's prose schema name makes a poor Rust
//! name. Hand-written code imports from this module, never from
//! `crate::generated`; when a spec bump renames a generated type, this
//! file is the first (often only) fix-site.
//!
//! # Discriminated unions
//!
//! The spec's polymorphic schemas (`NetworkDetails`, `ClientDetails`,
//! `ClientActionRequest`, ...) are generated as enums with one newtype
//! variant per concrete type. Each variant's payload carries its tag
//! field as a single-value enum (e.g. `UnmanagedNetworkDetails::management`),
//! so the tag round-trips on the wire and exactly one variant matches
//! when decoding.

pub use crate::generated::{
    // GET /v1/info
    ApplicationInfo,
    // POST /v1/sites/{siteId}/clients/{clientId}/actions
    ClientActionRequest,
    GatewayManagedNetworkDetails,
    GuestAccessAuthorizationRequest,
    GuestAccessAuthorizationRequestAction,
    GuestAccessUnauthorizationRequest,
    GuestAccessUnauthorizationRequestAction,
    // GET /v1/sites/{siteId}/networks/{networkId}
    NetworkDetails,
    SwitchManagedNetworkDetails,
    UnmanagedNetworkDetails,
};

#[cfg(test)]
mod tests {
    //! Page DTOs are deliberately not re-exported (phase 4 wraps every
    //! list in one generic `Page<T>`), so their shape is checked here,
    //! inside the crate.

    use crate::generated::{NetworkOverview, NetworkOverviewPage};

    #[test]
    fn page_dto_decodes_the_shared_envelope() {
        let page: NetworkOverviewPage =
            serde_json::from_str(include_str!("../tests/fixtures/networks_page.json"))
                .expect("page fixture decodes");
        assert_eq!(
            (page.offset, page.limit, page.count, page.total_count),
            (0, 25, 2, 2)
        );
        assert!(matches!(
            page.data.as_slice(),
            [
                NetworkOverview::GatewayManagedNetworkOverview(_),
                NetworkOverview::UnmanagedNetworkOverview(_),
            ]
        ));
    }
}
