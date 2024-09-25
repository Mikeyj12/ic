#![no_main]
use ic_management_canister_types::{CanisterInstallMode, CanisterSettingsArgsBuilder};
use ic_registry_subnet_type::SubnetType;
use ic_state_machine_tests::StateMachineBuilder;
use ic_types::Cycles;
use libfuzzer_sys::fuzz_target;
use wasm_fuzzers::ic_wasm::ICWasmModule;

// This fuzz tries to execute system API call.
//
// The fuzz test is only compiled but not executed by CI.
//
// To execute the fuzzer run
// bazel run --config=fuzzing //rs/execution_environment/fuzz:execute_system_api_call

fuzz_target!(|module: ICWasmModule| {
    let wasm = module.module.to_bytes();
    let env = StateMachineBuilder::new()
        .with_subnet_type(SubnetType::Application)
        .no_dts()
        .with_checkpoints_enabled(false)
        .build();
    let canister_id = env.create_canister_with_cycles(
        None,
        Cycles::from(100_000_000_000_u128),
        Some(CanisterSettingsArgsBuilder::new().build()),
    );
    env.install_wasm_in_mode(canister_id, CanisterInstallMode::Install, wasm, vec![])
        .unwrap();

    let _ = env.execute_ingress(canister_id, "update", vec![]);
});
