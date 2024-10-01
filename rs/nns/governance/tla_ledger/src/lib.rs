use async_trait::async_trait;
use dfn_core::CanisterId;
use ic_nervous_system_canisters::ledger::IcpLedgerCanister;
use ic_nervous_system_common::ledger::IcpLedger;
use ic_nervous_system_common::NervousSystemError;
use icp_ledger::{AccountIdentifier, Subaccount as IcpSubaccount, Tokens};

use ic_nervous_system_tla::{
    self as tla, account_to_tla, opt_subaccount_to_tla, store::TLA_INSTRUMENTATION_STATE,
    tla_log_request, tla_log_response, Destination, ToTla,
};
use std::collections::BTreeMap;

pub struct LoggingIcpLedgerCanister {
    ledger: IcpLedgerCanister,
}

impl LoggingIcpLedgerCanister {
    pub fn new(id: CanisterId) -> Self {
        LoggingIcpLedgerCanister {
            ledger: IcpLedgerCanister::new(id),
        }
    }
}

#[async_trait]
impl IcpLedger for LoggingIcpLedgerCanister {
    async fn transfer_funds(
        &self,
        amount_e8s: u64,
        fee_e8s: u64,
        from_subaccount: Option<IcpSubaccount>,
        to: AccountIdentifier,
        memo: u64,
    ) -> Result<u64, NervousSystemError> {
        tla_log_request!(
            "WaitForTransfer",
            Destination::new("ledger"),
            "Transfer",
            tla::TlaValue::Record(BTreeMap::from([
                ("amount".to_string(), amount_e8s.to_tla_value()),
                ("fee".to_string(), fee_e8s.to_tla_value()),
                ("from".to_string(), opt_subaccount_to_tla(&from_subaccount)),
                ("to".to_string(), account_to_tla(to)),
            ]))
        );

        let result = self
            .ledger
            .transfer_funds(amount_e8s, fee_e8s, from_subaccount, to, memo)
            .await;

        tla_log_response!(
            Destination::new("ledger"),
            if result.is_err() {
                tla::TlaValue::Variant {
                    tag: "Fail".to_string(),
                    value: Box::new(tla::TlaValue::Constant("UNIT".to_string())),
                }
            } else {
                tla::TlaValue::Variant {
                    tag: "TransferOk".to_string(),
                    value: Box::new(tla::TlaValue::Constant("UNIT".to_string())),
                }
            }
        );

        result
    }

    async fn total_supply(&self) -> Result<Tokens, NervousSystemError> {
        self.ledger.total_supply().await
    }

    async fn account_balance(
        &self,
        account: AccountIdentifier,
    ) -> Result<Tokens, NervousSystemError> {
        self.ledger.account_balance(account).await
    }

    fn canister_id(&self) -> CanisterId {
        self.ledger.canister_id()
    }
}
