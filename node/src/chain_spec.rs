use sp_core::{crypto::UncheckedInto};
use node_template_runtime::{
	AccountId, AuraConfig, BalancesConfig, GenesisConfig, GrandpaConfig,
	SudoConfig, SystemConfig, WASM_BINARY
};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sc_service::ChainType;
use node_template_runtime::ContractsConfig;
use hex_literal::hex;

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

pub fn local_testnet_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm binary not available".to_string())?;

	Ok(ChainSpec::from_genesis(
		// Name
		"subgame",
		// ID
		"subgame",
		ChainType::Local,
		move || testnet_genesis(
			wasm_binary,
			// Initial PoA authorities
			vec![(
				hex!["50a03202347d4cb254b62c476aef48b1cf0f44603913674b805d19e1a8987208"].unchecked_into(),
				hex!["e3bda27d551a3fbff7e58f4388ecd33ffccdf877c55bf559e32f3076a61beb45"].unchecked_into(),
			),(
				hex!["088b874d19c72096c1a5b1b7789592a980d712a79c5e5d4b5493f1b4bb3d6151"].unchecked_into(),
				hex!["7e41726e3d43b84d44c02e92daa7ab51207bfdd0d3202af5aa4ef8505e7e1f51"].unchecked_into(),
			)],
			// Sudo account
			hex!["f03bb9ee7cba9bf90724ac5bd90fcd9553969448dbd4cd3c88b0ee41a062c515"].into(),
			// Pre-funded accounts
			vec![
				hex!["f03bb9ee7cba9bf90724ac5bd90fcd9553969448dbd4cd3c88b0ee41a062c515"].into()
			],
			true,
		),
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		None,
		// Properties
		None,
		// Extensions
		None,
	))
}

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AuraId, GrandpaId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	enable_println: bool,
) -> GenesisConfig {
	GenesisConfig {
		frame_system: Some(SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(BalancesConfig {
			// Configure endowed accounts with initial balance of 1 << 60.
			balances: endowed_accounts.iter().cloned().map(|k|(k, 1000000000000000000)).collect(),
		}),
		pallet_aura: Some(AuraConfig {
			authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect(),
		}),
		pallet_grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect(),
		}),
		pallet_sudo: Some(SudoConfig {
			// Assign network admin rights.
			key: root_key,
		}),
		/*** Pallet Contracts ***/
		pallet_contracts: Some(ContractsConfig {
			current_schedule: pallet_contracts::Schedule {
				enable_println,
				..Default::default()
			},
		}),
		/*** Pallet Contracts ***/
	}
}