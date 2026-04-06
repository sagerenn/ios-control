use ble_helper::{backend::HostCapability, state::helper_state_from_capability};

#[test]
fn unsupported_capability_maps_to_unavailable_state() {
    let capability = HostCapability::unsupported("bluetooth peripheral role not supported");

    let state = helper_state_from_capability(&capability, false);

    assert_eq!(state.phase, "Unavailable");
    assert!(!state.execute_ready);
    assert_eq!(state.notes, vec!["bluetooth peripheral role not supported"]);
}

#[test]
fn supported_capability_without_bond_maps_to_ready_to_advertise() {
    let capability = HostCapability::supported("linux");

    let state = helper_state_from_capability(&capability, false);

    assert_eq!(state.phase, "ReadyToAdvertise");
    assert_eq!(
        state.checklist,
        vec!["Enable Bluetooth", "Pair the device when it appears"]
    );
}
