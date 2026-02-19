#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String};

pub mod dispute;
use dispute::{
    Dispute, DisputeOutcome, DisputeResolution, DisputeStatus, DisputeType,
    generate_dispute_id, store_dispute, get_dispute, get_dispute_ids_by_attestation,
    get_dispute_ids_by_challenger, add_dispute_to_attestation_index, add_dispute_to_challenger_index,
    validate_dispute_eligibility, validate_dispute_resolution, validate_dispute_closure,
};

#[contract]
pub struct AttestationContract;

#[contractimpl]
impl AttestationContract {
    /// Submit a revenue attestation: store merkle root and metadata for (business, period).
    /// Prevents overwriting existing attestation for the same period (idempotency).
    pub fn submit_attestation(
        env: Env,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
        timestamp: u64,
        version: u32,
    ) {
        let key = (business, period);
        if env.storage().instance().has(&key) {
            panic!("attestation already exists for this business and period");
        }
        let data = (merkle_root, timestamp, version);
        env.storage().instance().set(&key, &data);
    }

    /// Return stored attestation for (business, period) if any.
    pub fn get_attestation(
        env: Env,
        business: Address,
        period: String,
    ) -> Option<(BytesN<32>, u64, u32)> {
        let key = (business, period);
        env.storage().instance().get(&key)
    }

    /// Verify that an attestation exists and matches the given merkle root.
    pub fn verify_attestation(
        env: Env,
        business: Address,
        period: String,
        merkle_root: BytesN<32>,
    ) -> bool {
        if let Some((stored_root, _ts, _ver)) = Self::get_attestation(env.clone(), business, period)
        {
            stored_root == merkle_root
        } else {
            false
        }
    }

    /// Open a new dispute for an existing attestation
    /// 
    /// # Arguments
    /// * `challenger` - Address of the party challenging the attestation
    /// * `business` - Business address associated with the attestation
    /// * `period` - Period of the attestation being disputed
    /// * `dispute_type` - Type of dispute being raised
    /// * `evidence` - Evidence or description supporting the dispute
    /// 
    /// # Returns
    /// The ID of the newly created dispute
    /// 
    /// # Panics
    /// * If no attestation exists for the given business and period
    /// * If challenger already has an open dispute for this attestation
    /// * If challenger is not authorized to open disputes
    pub fn open_dispute(
        env: Env,
        challenger: Address,
        business: Address,
        period: String,
        dispute_type: DisputeType,
        evidence: String,
    ) -> u64 {
        // Validate eligibility
        validate_dispute_eligibility(&env, &challenger, &business, &period)
            .unwrap_or_else(|e| panic!("{}", e));
        
        // Generate new dispute ID
        let dispute_id = generate_dispute_id(&env);
        
        // Create dispute record
        let dispute = Dispute {
            id: dispute_id,
            challenger: challenger.clone(),
            business: business.clone(),
            period: period.clone(),
            status: DisputeStatus::Open,
            dispute_type,
            evidence,
            timestamp: env.ledger().timestamp(),
            resolution: None,
        };
        
        // Store dispute
        store_dispute(&env, &dispute);
        
        // Update indices
        add_dispute_to_attestation_index(&env, &business, &period, dispute_id);
        add_dispute_to_challenger_index(&env, &challenger, dispute_id);
        
        dispute_id
    }

    /// Resolve an open dispute with an outcome
    /// 
    /// # Arguments
    /// * `dispute_id` - ID of the dispute to resolve
    /// * `resolver` - Address of the party resolving the dispute
    /// * `outcome` - Outcome of the dispute resolution
    /// * `notes` - Optional notes about the resolution
    /// 
    /// # Panics
    /// * If dispute doesn't exist
    /// * If dispute is not in Open status
    /// * If resolver is not authorized to resolve disputes
    pub fn resolve_dispute(
        env: Env,
        dispute_id: u64,
        resolver: Address,
        outcome: DisputeOutcome,
        notes: String,
    ) {
        // Validate resolution eligibility
        let mut dispute = validate_dispute_resolution(&env, dispute_id, &resolver)
            .unwrap_or_else(|e| panic!("{}", e));
        
        // Create resolution record
        let resolution = DisputeResolution {
            resolver: resolver.clone(),
            outcome,
            timestamp: env.ledger().timestamp(),
            notes,
        };
        
        // Update dispute status and resolution
        dispute.status = DisputeStatus::Resolved;
        dispute.resolution = Some(resolution);
        
        // Store updated dispute
        store_dispute(&env, &dispute);
    }

    /// Close a resolved dispute, making it final
    /// 
    /// # Arguments
    /// * `dispute_id` - ID of the dispute to close
    /// 
    /// # Panics
    /// * If dispute doesn't exist
    /// * If dispute is not in Resolved status
    pub fn close_dispute(env: Env, dispute_id: u64) {
        // Validate closure eligibility
        let mut dispute = validate_dispute_closure(&env, dispute_id)
            .unwrap_or_else(|e| panic!("{}", e));
        
        // Update dispute status
        dispute.status = DisputeStatus::Closed;
        
        // Store updated dispute
        store_dispute(&env, &dispute);
    }

    /// Get details of a specific dispute
    /// 
    /// # Arguments
    /// * `dispute_id` - ID of the dispute to retrieve
    /// 
    /// # Returns
    /// Option containing the dispute details, or None if not found
    pub fn get_dispute(env: Env, dispute_id: u64) -> Option<Dispute> {
        get_dispute(&env, dispute_id)
    }

    /// Get all dispute IDs for a specific attestation
    /// 
    /// # Arguments
    /// * `business` - Business address
    /// * `period` - Period string
    /// 
    /// # Returns
    /// Vector of dispute IDs associated with this attestation
    pub fn get_disputes_by_attestation(env: Env, business: Address, period: String) -> soroban_sdk::Vec<u64> {
        get_dispute_ids_by_attestation(&env, &business, &period)
    }

    /// Get all dispute IDs opened by a specific challenger
    /// 
    /// # Arguments
    /// * `challenger` - Address of the challenger
    /// 
    /// # Returns
    /// Vector of dispute IDs opened by this challenger
    pub fn get_disputes_by_challenger(env: Env, challenger: Address) -> soroban_sdk::Vec<u64> {
        get_dispute_ids_by_challenger(&env, &challenger)
    }
}

mod test;
mod dispute_test;
