use std::str::FromStr;

use alloy::{
    contract,
    network::{Ethereum, EthereumWallet},
    primitives::{Address, FixedBytes, Uint},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, PendingTransactionBuilder, Provider, ProviderBuilder, RootProvider,
        WalletProvider,
    },
    signers::local::LocalSigner,
    sol_types::SolEvent,
    transports::http::{reqwest::Url, Client, Http},
};

use crate::types::*;

type EthereumHttpProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<Http<Client>>,
    Http<Client>,
    Ethereum,
>;

type LivenessContract = Liveness::LivenessInstance<
    Http<Client>,
    FillProvider<
        JoinFill<
            JoinFill<
                Identity,
                JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
            >,
            WalletFiller<EthereumWallet>,
        >,
        RootProvider<Http<Client>>,
        Http<Client>,
        Ethereum,
    >,
>;

pub struct Publisher {
    provider: EthereumHttpProvider,
    liveness_contract: LivenessContract,
}

pub struct ValidationInfo {
    platform: String,
    service_provider: String,
    validation_service_manager: Address,
}

impl Publisher {
    /// Create a new [`Publisher`] instance to call contract functions and send
    /// transactions.
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    /// ```
    pub fn new(
        ethereum_rpc_url: impl AsRef<str>,
        signing_key: impl AsRef<str>,
        liveness_contract_address: impl AsRef<str>,
    ) -> Result<Self, PublisherError> {
        let rpc_url: Url = ethereum_rpc_url
            .as_ref()
            .parse()
            .map_err(|error| PublisherError::ParseEthereumRpcUrl(Box::new(error)))?;

        let signer =
            LocalSigner::from_str(signing_key.as_ref()).map_err(PublisherError::ParseSigningKey)?;

        let wallet = EthereumWallet::new(signer.clone());

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(rpc_url);

        let liveness_contract_address = Address::from_str(liveness_contract_address.as_ref())
            .map_err(|error| {
                PublisherError::ParseAddress(liveness_contract_address.as_ref().to_owned(), error)
            })?;
        let liveness_contract =
            Liveness::LivenessInstance::new(liveness_contract_address, provider.clone());

        Ok(Self {
            provider,
            liveness_contract,
        })
    }

    /// Get the address for the wallet used by [`Publisher`].
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let my_address = publisher.address();
    /// ```
    pub fn address(&self) -> Address {
        self.provider.default_signer_address()
    }

    /// Get the latest Ethereum block number available.
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let ethereum_latest_block_number = publisher.get_block_number().await.unwrap();
    /// ```
    pub async fn get_block_number(&self) -> Result<u64, PublisherError> {
        let block_number = self
            .provider
            .get_block_number()
            .await
            .map_err(PublisherError::GetBlockNumber)?;

        Ok(block_number)
    }

    /// # TODO:
    /// Fix the block margin return type to one of the smaller types.
    ///
    /// Get the block margin specified by the contract. Use the block margin to
    /// check the validity of the block number passed to the
    /// [`get_sequencer_list()`] function.
    ///
    /// # Examples
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let block_margin = publisher.get_block_margin().await.unwrap();
    /// ```
    pub async fn get_block_margin(&self) -> Result<Uint<256, 4>, PublisherError> {
        let block_margin = self
            .liveness_contract
            .BLOCK_MARGIN()
            .call()
            .await
            .map_err(PublisherError::GetBlockMargin)?
            ._0;

        Ok(block_margin)
    }

    /// Send transaction to initialize the cluster and wait for the event
    /// to return.
    ///
    /// # Examples
    ///
    /// ```
    /// use liveness_radius::publisher::Publisher;
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let event = publisher.initialize_cluster("radius").await?;
    ///
    /// println!(r"Owner: {}\Cluster ID: {}", event.owner, event.clusterId);
    /// ```
    pub async fn initialize_cluster(
        &self,
        cluster_id: impl AsRef<str>,
        max_sequencer_number: Uint<256, 4>,
    ) -> Result<Liveness::InitializedCluster, PublisherError> {
        let contract_call = self
            .liveness_contract
            .initializeCluster(cluster_id.as_ref().to_string(), max_sequencer_number);
        let pending_transaction = contract_call.send().await;
        let event: Liveness::InitializedCluster = self
            .extract_event_from_pending_transaction(pending_transaction)
            .await
            .map_err(PublisherError::InitializedCluster)?;

        Ok(event)
    }

    /// Send transaction to add the rollup and wait for the event
    /// to return.
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let event = publisher.add_rollup("radius", "rollup_1", "0x67d269191c92Caf3cD7723F116c85e6E9bf55933", "txHash", {platform: "ethereum", serviceProvider: "eigen_layer"}).await?;
    ///
    /// println!(
    ///     "Cluster ID: {}\Rollup ID: {}\Rollup Owner: {}",
    ///     event.clusterId, event.rollupId, event.rollupOwnerAddress
    /// );
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub async fn add_rollup(
        &self,
        cluster_id: impl AsRef<str>,
        rollup_id: impl AsRef<str>,
        rollup_type: impl AsRef<str>,
        rollup_owner_address: impl AsRef<str>,
        order_commitment_type: impl AsRef<str>,
        encrypted_transaction_type: impl AsRef<str>,
        validation_info: ValidationInfo,
        executor_address: impl AsRef<str>,
    ) -> Result<Liveness::AddedRollup, PublisherError> {
        let rollup_owner_address =
            Address::from_str(rollup_owner_address.as_ref()).map_err(|error| {
                PublisherError::ParseAddress(rollup_owner_address.as_ref().to_owned(), error)
            })?;

        let executor_address = Address::from_str(executor_address.as_ref()).map_err(|error| {
            PublisherError::ParseAddress(executor_address.as_ref().to_owned(), error)
        })?;

        let validation_info = ILivenessRadius::ValidationInfo {
            platform: validation_info.platform,
            serviceProvider: validation_info.service_provider,
            validationServiceManager: validation_info.validation_service_manager,
        };

        let new_rollup = ILivenessRadius::NewRollup {
            rollupId: rollup_id.as_ref().to_string(),
            owner: rollup_owner_address,
            rollupType: rollup_type.as_ref().to_string(),
            encryptedTransactionType: encrypted_transaction_type.as_ref().to_string(),
            validationInfo: validation_info,
            orderCommitmentType: order_commitment_type.as_ref().to_string(),
            executor: executor_address,
        };

        let contract_call = self
            .liveness_contract
            .addRollup(cluster_id.as_ref().to_string(), new_rollup);

        let pending_transaction = contract_call.send().await;
        let event: Liveness::AddedRollup = self
            .extract_event_from_pending_transaction(pending_transaction)
            .await
            .map_err(PublisherError::AddedRollup)?;

        Ok(event)
    }

    /// Send transaction to add rollup executor and wait for the event
    /// to return.
    ///
    /// # Examples
    ///
    /// ```
    /// use liveness_radius::publisher::Publisher;
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let event = publisher
    ///     .register_rollup_executor(
    ///         "radius",
    ///         "rollup_1",
    ///         "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    ///     )
    ///     .await?;
    ///
    /// println!(
    ///     r"Cluster ID: {}\Rollup ID: {}\Rollup Executor: {}",
    ///     event.clusterId, event.rollupId, event.rollupExecutorAddress
    /// );
    /// ```
    pub async fn register_rollup_executor(
        &self,
        cluster_id: impl AsRef<str>,
        rollup_id: impl AsRef<str>,
        rollup_executor_address: impl AsRef<str>,
    ) -> Result<Liveness::RegisteredRollupExecutor, PublisherError> {
        let rollup_executor_address =
            Address::from_str(rollup_executor_address.as_ref()).map_err(|error| {
                PublisherError::ParseAddress(rollup_executor_address.as_ref().to_owned(), error)
            })?;

        let contract_call = self.liveness_contract.registerRollupExecutor(
            cluster_id.as_ref().to_string(),
            rollup_id.as_ref().to_string(),
            rollup_executor_address,
        );

        let pending_transaction = contract_call.send().await;
        let event: Liveness::RegisteredRollupExecutor = self
            .extract_event_from_pending_transaction(pending_transaction)
            .await
            .map_err(PublisherError::RegisteredRollupExecutor)?;

        Ok(event)
    }

    /// Register the current [`Publisher`] instance as a sequencer of the
    /// cluster. The address of the registered sequencer is equivalent
    /// to that of self.address().
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let event = publisher
    ///     .register_sequencer("0xdd45347e5d10daaadb40f185225fc8d860d2888b5c411aca387e17a265e2f491")
    ///     .await
    ///     .unwrap();
    ///
    /// assert!(event.sequencerAddress == publisher.address());
    /// ```
    pub async fn register_sequencer(
        &self,
        cluster_id: impl AsRef<str>,
    ) -> Result<Liveness::RegisteredSequencer, PublisherError> {
        let contract_call = self
            .liveness_contract
            .registerSequencer(cluster_id.as_ref().to_string());
        let pending_transaction = contract_call.send().await;
        let event: Liveness::RegisteredSequencer = self
            .extract_event_from_pending_transaction(pending_transaction)
            .await
            .map_err(PublisherError::RegisteredSequencer)?;

        Ok(event)
    }

    /// Deregister the publisher's address from the cluster.
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let event = publisher
    ///     .deregister_sequencer("0xdd45347e5d10daaadb40f185225fc8d860d2888b5c411aca387e17a265e2f491")
    ///     .await
    ///     .unwrap();
    ///
    /// assert!(event.sequencerAddress == publisher.address());
    /// ```
    pub async fn deregister_sequencer(
        &self,
        cluster_id: impl AsRef<str>,
    ) -> Result<Liveness::DeregisteredSequencer, PublisherError> {
        let contract_call = self
            .liveness_contract
            .deregisterSequencer(cluster_id.as_ref().to_string());
        let pending_transaction = contract_call.send().await;
        let event: Liveness::DeregisteredSequencer = self
            .extract_event_from_pending_transaction(pending_transaction)
            .await
            .map_err(PublisherError::DeregisteredSequencer)?;

        Ok(event)
    }

    /// Get the addresses of registered sequencers in a given cluster for a
    /// given block number.
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )?;
    ///
    /// let block_number = publisher.get_block_number().await.unwrap();
    /// let sequencer_list = publisher
    ///     .get_sequencer_list(cluster_id, block_number)
    ///     .await
    ///     .unwrap();
    ///
    /// println!("{:?}", sequencer_list);
    /// ```
    pub async fn get_sequencer_list(
        &self,
        cluster_id: impl AsRef<str>,
        block_number: u64,
    ) -> Result<Vec<Address>, PublisherError> {
        let sequencer_list = self
            .liveness_contract
            .getSequencers(cluster_id.as_ref().to_string())
            .call()
            .block(block_number.into())
            .await
            .map_err(PublisherError::GetSequencers)?
            ._0;

        Ok(sequencer_list)
    }

    /// Get the addresses of registered rollups in a given cluster for a
    /// given block number.
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )?;
    ///
    /// let block_number = publisher.get_block_number().await.unwrap();
    /// let executor_list = publisher
    ///     .get_executor_list(cluster_id, rollup_id, block_number)
    ///     .await
    ///     .unwrap();
    ///
    /// println!("{:?}", executor_list);
    /// ```
    pub async fn get_executor_list(
        &self,
        cluster_id: impl AsRef<str>,
        rollup_id: impl AsRef<str>,
        block_number: u64,
    ) -> Result<Vec<Address>, PublisherError> {
        let executor_list = self
            .liveness_contract
            .getExecutors(
                cluster_id.as_ref().to_string(),
                rollup_id.as_ref().to_string(),
            )
            .call()
            .block(block_number.into())
            .await
            .map_err(PublisherError::GetSequencers)?
            ._0;

        let filtered_list: Vec<Address> = executor_list
            .into_iter()
            .filter(|sequencer_address| !sequencer_address.is_zero())
            .collect();

        Ok(filtered_list)
    }

    pub async fn get_rollup_info_list(
        &self,
        cluster_id: impl AsRef<str>,
        block_number: u64,
    ) -> Result<Vec<ILivenessRadius::Rollup>, PublisherError> {
        let executor_list = self
            .liveness_contract
            .getRollups(cluster_id.as_ref().to_string())
            .call()
            .block(block_number.into())
            .await
            .map_err(PublisherError::GetRollups)?
            ._0;

        Ok(executor_list)
    }

    pub async fn get_rollup_info(
        &self,
        cluster_id: impl AsRef<str>,
        rollup_id: impl AsRef<str>,
        block_number: u64,
    ) -> Result<ILivenessRadius::Rollup, PublisherError> {
        let rollup_info = self
            .liveness_contract
            .getRollup(
                cluster_id.as_ref().to_string(),
                rollup_id.as_ref().to_string(),
            )
            .call()
            .block(block_number.into())
            .await
            .map_err(PublisherError::GetRollup)?
            ._0;

        Ok(rollup_info)
    }

    /// # TODO:
    /// Fix the max sequencer number return type to one of the smaller types.
    ///
    /// # Examples
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let max_sequencer_number = publisher
    ///     .get_max_sequencer_number(cluster_id)
    ///     .await
    ///     .unwrap();
    /// ```
    pub async fn get_max_sequencer_number(
        &self,
        cluster_id: impl AsRef<str>,
    ) -> Result<Uint<256, 4>, PublisherError> {
        let max_sequencer_number = self
            .liveness_contract
            .getMaxSequencerNumber(cluster_id.as_ref().to_string())
            .call()
            .await
            .map_err(PublisherError::GetBlockMargin)?
            ._0;

        Ok(max_sequencer_number)
    }

    pub async fn is_added_rollup(
        &self,
        cluster_id: impl AsRef<str>,
        rollup_id: impl AsRef<str>,
    ) -> Result<bool, PublisherError> {
        let is_rollup_added: bool = self
            .liveness_contract
            .isRollupAdded(
                cluster_id.as_ref().to_string(),
                rollup_id.as_ref().to_string(),
            )
            .call()
            .await
            .map_err(PublisherError::IsRegistered)?
            ._0;

        Ok(is_rollup_added)
    }

    pub async fn is_rollup_executor_registered(
        &self,
        cluster_id: impl AsRef<str>,
        rollup_id: impl AsRef<str>,
        executor_address: Address,
    ) -> Result<bool, PublisherError> {
        let is_rollup_executor_registered: bool = self
            .liveness_contract
            .isRollupExecutorRegistered(
                cluster_id.as_ref().to_string(),
                rollup_id.as_ref().to_string(),
                executor_address,
            )
            .call()
            .await
            .map_err(PublisherError::IsRegistered)?
            ._0;

        Ok(is_rollup_executor_registered)
    }

    /// Check if the current publisher is registered as a sequencer in the
    /// cluster.
    ///
    /// # Examples
    ///
    /// ```
    /// let publisher = Publisher::new(
    ///     "http://127.0.0.1:8545",
    ///     "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ///     "0x67d269191c92Caf3cD7723F116c85e6E9bf55933",
    /// )
    /// .unwrap();
    ///
    /// let is_registered_sequencer = publisher.is_registered_sequencer(cluster_id).await.unwrap();
    ///
    /// assert!(is_registered_sequencer == true);
    /// ```
    pub async fn is_registered_sequencer(
        &self,
        cluster_id: impl AsRef<str>,
    ) -> Result<bool, PublisherError> {
        let is_registered_sequencer: bool = self
            .liveness_contract
            .isSequencerRegistered(cluster_id.as_ref().to_string(), self.address())
            .call()
            .await
            .map_err(PublisherError::IsRegistered)?
            ._0;

        Ok(is_registered_sequencer)
    }

    async fn extract_event_from_pending_transaction<T>(
        &self,
        pending_transaction: Result<
            PendingTransactionBuilder<Http<Client>, Ethereum>,
            contract::Error,
        >,
    ) -> Result<T, TransactionError>
    where
        T: SolEvent,
    {
        let transaction_receipt = pending_transaction
            .map_err(TransactionError::SendTransaction)?
            .get_receipt()
            .await
            .map_err(TransactionError::GetReceipt)?;

        match transaction_receipt.as_ref().is_success() {
            true => {
                let log = transaction_receipt
                    .as_ref()
                    .logs()
                    .first()
                    .ok_or(TransactionError::EmptyLogs)?
                    .log_decode::<T>()
                    .map_err(TransactionError::DecodeLogData)?;

                Ok(log.inner.data)
            }
            false => Err(TransactionError::FailedTransaction(
                transaction_receipt.transaction_hash,
            )),
        }
    }
}

#[derive(Debug)]
pub enum TransactionError {
    SendTransaction(alloy::contract::Error),
    GetReceipt(alloy::providers::PendingTransactionError),
    FailedTransaction(FixedBytes<32>),
    EmptyLogs,
    DecodeLogData(alloy::sol_types::Error),
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TransactionError {}

#[derive(Debug)]
pub enum PublisherError {
    ParseEthereumRpcUrl(Box<dyn std::error::Error>),
    ParseSigningKey(alloy::signers::local::LocalSignerError),
    ParseAddress(String, alloy::hex::FromHexError),
    GetBlockNumber(alloy::transports::RpcError<alloy::transports::TransportErrorKind>),
    GetBlockMargin(alloy::contract::Error),
    InitializedCluster(TransactionError),
    AddedRollup(TransactionError),
    RegisteredRollupExecutor(TransactionError),
    RegisteredSequencer(TransactionError),
    DeregisteredSequencer(TransactionError),
    GetSequencers(alloy::contract::Error),
    GetRollups(alloy::contract::Error),
    GetRollup(alloy::contract::Error),
    IsRegistered(alloy::contract::Error),
}

impl std::fmt::Display for PublisherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PublisherError {}
