#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Vec};

pub mod dynamic_fees;
pub use dynamic_fees::{compute_fee, DataKey, FeeConfig};

#[cfg(test)]
mod test;
#[cfg(test)]
mod dynamic_fees_test;

const ANOMALY_KEY_TAG: u32 = 1;
const ADMIN_KEY_TAG: (u32,) = (2,);
const AUTHORIZED_KEY_TAG: u32 = 3;
const ANOMALY_SCORE_MAX: u32 = 100;

#[contract]
pub struct AttestationContract;

#[contractimpl]
impl AttestationContract {
    // ── Initialization ──────────────────────────────────────────────

    /// One-time contract initialization. Sets the admin address.
    ///
    /// Must be called before any admin-gated method. The caller must
    /// authorize as `admin`.
    pub fn initialize(env: Env, admin: Address) {
        if dynamic_fees::is_initialized(&env) {
            panic!("already initialized");
        }
        admin.require_auth();
        dynamic_fees::set_admin(&env, &admin);
    }

    // ── Admin: Fee configuration ────────────────────────────────────

    /// Configure or update the core fee schedule.
    ///
    /// * `token`    – Token contract address for fee payment.
    /// * `collector` – Address that receives fees.
    /// * `base_fee` – Base fee in token smallest units.
    /// * `enabled`  – Master switch for fee collection.
    pub fn configure_fees(
        env: Env,
        token: Address,
        collector: Address,
        base_fee: i128,
        enabled: bool,
    ) {
        dynamic_fees::require_admin(&env);
        assert!(base_fee >= 0, "base_fee must be non-negative");
        let config = FeeConfig {
            token,
            collector,
            base_fee,
            enabled,
        };
        dynamic_fees::set_fee_config(&env, &config);
    }

    /// Set the discount (in basis points, 0–10 000) for a tier level.
    ///
    /// * Tier 0 = Standard (default for all businesses).
    /// * Tier 1 = Professional.
    /// * Tier 2 = Enterprise.
    ///
    /// Higher tiers are allowed; the scheme is open-ended.
    pub fn set_tier_discount(env: Env, tier: u32, discount_bps: u32) {
        dynamic_fees::require_admin(&env);
        dynamic_fees::set_tier_discount(&env, tier, discount_bps);
    }

    /// Assign a business address to a fee tier.
    pub fn set_business_tier(env: Env, business: Address, tier: u32) {
        dynamic_fees::require_admin(&env);
        dynamic_fees::set_business_tier(&env, &business, tier);
    }

    /// Set volume discount brackets.
    ///
    /// `thresholds` and `discounts` must be equal-length vectors.
    /// Thresholds must be in strictly ascending order.
    /// Each discount is in basis points (0–10 000).
    ///
    /// Example: thresholds `[10, 50, 100]`, discounts `[500, 1000, 2000]`
    /// means 5 % off after 10 attestations, 10 % after 50, 20 % after 100.
    pub fn set_volume_brackets(env: Env, thresholds: Vec<u64>, discounts: Vec<u32>) {
        dynamic_fees::require_admin(&env);
        dynamic_fees::set_volume_brackets(&env, &thresholds, &discounts);
    }

    /// Toggle fee collection on or off without changing other config.
    pub fn set_fee_enabled(env: Env, enabled: bool) {
        dynamic_fees::require_admin(&env);
        let mut config = dynamic_fees::get_fee_config(&env).expect("fees not configured");
        config.enabled = enabled;
        dynamic_fees::set_fee_config(&env, &config);
    }

    // ── Core attestation methods ────────────────────────────────────

    /// Submit a revenue attestation.
    ///
    /// Stores the Merkle root, timestamp, and version for the given
    /// (business, period) pair. If fees are enabled the caller pays the
    /// calculated fee (base fee adjusted by tier and volume discounts)
    /// in the configured token. The business address must authorize the
    /// call.
    ///
    /// Panics if an attestation already exists for the same
    /// (business, period).
    pub fn submit_attestation(
        env: Env,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
        timestamp: u64,
        version: u32,
    ) {
        business.require_auth();

        let key = DataKey::Attestation(business.clone(), period);
        if env.storage().instance().has(&key) {
            panic!("attestation already exists for this business and period");
        }

        // Collect fee (0 if fees disabled or not configured).
        let fee_paid = dynamic_fees::collect_fee(&env, &business);

        // Track volume for future discount calculations.
        dynamic_fees::increment_business_count(&env, &business);

        let data = (merkle_root, timestamp, version, fee_paid);
        env.storage().instance().set(&key, &data);
    }

    /// Return stored attestation for (business, period), if any.
    ///
    /// Returns `(merkle_root, timestamp, version, fee_paid)`.
    pub fn get_attestation(
        env: Env,
        business: Address,
        period: String,
    ) -> Option<(BytesN<32>, u64, u32, i128)> {
        let key = DataKey::Attestation(business, period);
        env.storage().instance().get(&key)
    }

    /// Verify that an attestation exists and its merkle root matches.
    pub fn verify_attestation(
        env: Env,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
    ) -> bool {
        if let Some((stored_root, _ts, _ver, _fee)) =
            Self::get_attestation(env.clone(), business, period)
        {
            stored_root == merkle_root
        } else {
            false
        }
    }

    /// One-time setup of the admin address. Admin is the single authorized updater of the
    /// authorized-analytics set. Anomaly data is stored under a separate instance key and
    /// never modifies attestation (merkle root, timestamp, version) storage.
    pub fn init(env: Env, admin: Address) {
        admin.require_auth();
        if env.storage().instance().has(&ADMIN_KEY_TAG) {
            panic!("admin already set");
        }
        env.storage().instance().set(&ADMIN_KEY_TAG, &admin);
    }

    /// Adds an address to the set of authorized updaters (analytics/oracle). Caller must be admin.
    pub fn add_authorized_analytics(env: Env, caller: Address, analytics: Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN_KEY_TAG)
            .expect("admin not set");
        if caller != admin {
            panic!("caller is not admin");
        }
        let key = (AUTHORIZED_KEY_TAG, analytics);
        env.storage().instance().set(&key, &());
    }

    /// Removes an address from the set of authorized updaters. Caller must be admin.
    pub fn remove_authorized_analytics(env: Env, caller: Address, analytics: Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN_KEY_TAG)
            .expect("admin not set");
        if caller != admin {
            panic!("caller is not admin");
        }
        let key = (AUTHORIZED_KEY_TAG, analytics);
        env.storage().instance().remove(&key);
    }

    /// Stores anomaly flags and risk score for an existing attestation. Only addresses in the
    /// authorized-analytics set (added by admin) may call this; updater must pass their address
    /// and authorize. flags: bitmask for anomaly conditions (semantics defined off-chain).
    /// score: risk score in [0, 100]; higher means higher risk. Panics if attestation missing or score > 100.
    pub fn set_anomaly(
        env: Env,
        updater: Address,
        business: Address,
        period: String,
        flags: u32,
        score: u32,
    ) {
        updater.require_auth();
        let key_auth = (AUTHORIZED_KEY_TAG, updater.clone());
        if !env.storage().instance().has(&key_auth) {
            panic!("updater not authorized");
        }
        let attest_key = (business.clone(), period.clone());
        if !env.storage().instance().has(&attest_key) {
            panic!("attestation does not exist for this business and period");
        }
        if score > ANOMALY_SCORE_MAX {
            panic!("score out of range");
        }
        let anomaly_key = (ANOMALY_KEY_TAG, business, period);
        env.storage().instance().set(&anomaly_key, &(flags, score));
    }

    /// Returns anomaly flags and risk score for (business, period) if set. For use by lenders.
    pub fn get_anomaly(
        env: Env,
        business: Address,
        period: String,
    ) -> Option<(u32, u32)> {
        let key = (ANOMALY_KEY_TAG, business, period);
        env.storage().instance().get(&key)
    }
}

mod test;
#[cfg(test)]
mod anomaly_test;

    // ── Read-only queries ───────────────────────────────────────────

    /// Return the current fee configuration, or None if not configured.
    pub fn get_fee_config(env: Env) -> Option<FeeConfig> {
        dynamic_fees::get_fee_config(&env)
    }

    /// Calculate the fee a business would pay for its next attestation.
    pub fn get_fee_quote(env: Env, business: Address) -> i128 {
        dynamic_fees::calculate_fee(&env, &business)
    }

    /// Return the tier assigned to a business (0 if unset).
    pub fn get_business_tier(env: Env, business: Address) -> u32 {
        dynamic_fees::get_business_tier(&env, &business)
    }

    /// Return the cumulative attestation count for a business.
    pub fn get_business_count(env: Env, business: Address) -> u64 {
        dynamic_fees::get_business_count(&env, &business)
    }

    /// Return the contract admin address.
    pub fn get_admin(env: Env) -> Address {
        dynamic_fees::get_admin(&env)
    }
}
