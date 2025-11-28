use odra::prelude::*;

use crate::error::LendingError;

/// Pyth Oracle implementation for Odra/Casper
/// Simplified version for Casper ecosystem

// Constants
pub const STALE_PRICE_THRESHOLD_SLOTS: u64 = 5;

#[odra::module]
pub struct PythOracle {
    // Store primitive types directly in separate mappings for simplicity
    price_values: Mapping<Address, i64>,
    price_confidences: Mapping<Address, u64>,
    price_statuses: Mapping<Address, u8>,
    price_publish_slots: Mapping<Address, u64>,
    price_exponents: Mapping<Address, i32>,
    
    product_price_addresses: Mapping<Address, Address>,
    product_attributes: Mapping<Address, Vec<(String, String)>>,
    
    approved_publishers: Mapping<Address, bool>,
    admin: Var<Address>,
    min_confidence_ratio: Var<u64>,
}

#[odra::module]
impl PythOracle {
    /// Initialize the Pyth oracle
    pub fn init(&mut self, admin: Address) {
        self.admin.set(admin);
        self.min_confidence_ratio.set(5); // 5% max confidence ratio
    }

    /// Update price for a token
    pub fn update_price(
        &mut self,
        token_address: Address,
        price: i64,
        confidence: u64,
        exponent: i32,
        status: u8,
        publish_slot: u64
    ) {
        let caller = self.env().caller();
        if self.admin.get().unwrap() != caller && !self.approved_publishers.get(&caller).unwrap_or(false) {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        // Status validation: 1 = Trading
        if status != 1 {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        // Store price data in separate mappings
        self.price_values.set(&token_address, price);
        self.price_confidences.set(&token_address, confidence);
        self.price_statuses.set(&token_address, status);
        self.price_publish_slots.set(&token_address, publish_slot);
        self.price_exponents.set(&token_address, exponent);

        self.env().emit_event(PriceUpdated {
            token_address,
            price,
            confidence,
            exponent,
            status,
            publisher: caller,
            slot: publish_slot,
        });
    }

    /// Add a new product
    pub fn add_product(
        &mut self,
        product_address: Address,
        price_address: Address,
        attributes: Vec<(String, String)>
    ) {
        let caller = self.env().caller();
        if self.admin.get().unwrap() != caller {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        self.product_price_addresses.set(&product_address, price_address);
        self.product_attributes.set(&product_address, attributes);

        self.env().emit_event(ProductAdded {
            product_address,
            price_address,
            added_by: caller,
        });
    }

    /// Add approved price publisher
    pub fn add_publisher(&mut self, publisher: Address) {
        let caller = self.env().caller();
        if self.admin.get().unwrap() != caller {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        self.approved_publishers.set(&publisher, true);

        self.env().emit_event(PublisherAdded {
            publisher,
            added_by: caller,
        });
    }

    /// Remove price publisher
    pub fn remove_publisher(&mut self, publisher: Address) {
        let caller = self.env().caller();
        if self.admin.get().unwrap() != caller {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        self.approved_publishers.set(&publisher, false);

        self.env().emit_event(PublisherRemoved {
            publisher,
            removed_by: caller,
        });
    }

    /// Get price for a token - returns raw u64 instead of Decimal for compatibility
    pub fn get_price(&self, token_address: Address, current_slot: u64) -> Option<u64> {
        let price = self.price_values.get(&token_address)?;
        let confidence = self.price_confidences.get(&token_address)?;
        let status = self.price_statuses.get(&token_address)?;
        let publish_slot = self.price_publish_slots.get(&token_address)?;
        let exponent = self.price_exponents.get(&token_address)?;

        // Check if price is stale
        let slots_elapsed = current_slot.checked_sub(publish_slot)?;

        if slots_elapsed >= STALE_PRICE_THRESHOLD_SLOTS {
            return None;
        }

        // Check price status (1 = Trading)
        if status != 1 {
            return None;
        }

        // Check confidence (price should not be too volatile)
        let price_value = price.unsigned_abs();
        let confidence_value = confidence;
        
        if price_value > 0 {
            // Simple integer-based confidence check
            // confidence_ratio = confidence / price
            if confidence_value > price_value.saturating_mul(self.min_confidence_ratio.get().unwrap()) / 100 {
                return None;
            }
        }

        // Convert price with proper exponent handling
        self.convert_pyth_price_to_u64(price, exponent)
    }

    /// Get price with confidence - returns raw u64 values
    pub fn get_price_with_confidence(&self, token_address: Address, current_slot: u64) -> Option<(u64, u64)> {
        let price = self.price_values.get(&token_address)?;
        let confidence = self.price_confidences.get(&token_address)?;
        let status = self.price_statuses.get(&token_address)?;
        let publish_slot = self.price_publish_slots.get(&token_address)?;
        let exponent = self.price_exponents.get(&token_address)?;

        // Check if price is stale
        let slots_elapsed = current_slot.checked_sub(publish_slot)?;

        if slots_elapsed >= STALE_PRICE_THRESHOLD_SLOTS {
            return None;
        }

        // Check price status
        if status != 1 {
            return None;
        }

        let market_price = self.convert_pyth_price_to_u64(price, exponent)?;
        let confidence_value = self.convert_pyth_price_to_u64(confidence as i64, exponent)?;

        Some((market_price, confidence_value))
    }

    /// Get product information
    pub fn get_product(&self, product_address: Address) -> Option<(Address, Vec<(String, String)>)> {
        let price_address = self.product_price_addresses.get(&product_address)?;
        let attributes = self.product_attributes.get(&product_address)?;
        Some((price_address, attributes))
    }

    /// Get quote currency from product attributes - returns raw bytes
    pub fn get_quote_currency(&self, product_address: Address) -> Option<Vec<u8>> {
        let attributes = self.product_attributes.get(&product_address)?;

        for (key, value) in &attributes {
            if key == "quote_currency" {
                return Some(value.as_bytes().to_vec());
            }
        }

        None
    }

    /// Set minimum confidence ratio (admin only)
    pub fn set_min_confidence_ratio(&mut self, ratio: u64) {
        let caller = self.env().caller();
        if self.admin.get().unwrap() != caller {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        self.min_confidence_ratio.set(ratio);

        self.env().emit_event(ConfidenceRatioUpdated {
            ratio,
            updated_by: caller,
        });
    }

    /// Transfer admin rights
    pub fn transfer_admin(&mut self, new_admin: Address) {
        let caller = self.env().caller();
        let current_admin = self.admin.get().unwrap();
        
        if current_admin != caller {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        self.admin.set(new_admin);

        self.env().emit_event(AdminTransferred {
            previous_admin: current_admin,
            new_admin,
        });
    }

    /// Check if address is approved publisher
    pub fn is_approved_publisher(&self, address: Address) -> bool {
        self.approved_publishers.get(&address).unwrap_or(false)
    }

    /// Get all supported tokens
    pub fn get_supported_tokens(&self) -> Vec<Address> {
        // Note: In Odra, we need to maintain a list separately since Mapping doesn't support keys()
        // For now, return empty vec - in practice you'd maintain a List of tokens
        vec![]
    }
}

impl PythOracle {
    /// Convert Pyth price to u64 with proper exponent handling
    fn convert_pyth_price_to_u64(&self, price: i64, exponent: i32) -> Option<u64> {
        if price < 0 {
            return None;
        }

        let price_unsigned = price.unsigned_abs();
        
        if exponent >= 0 {
            let exponent_u32 = exponent as u32;
            let multiplier = 10u64.checked_pow(exponent_u32)?;
            price_unsigned.checked_mul(multiplier)
        } else {
            let exponent_abs = exponent.unsigned_abs() as u32;
            let divisor = 10u64.checked_pow(exponent_abs)?;
            price_unsigned.checked_div(divisor)
        }
    }
}

// Events for Pyth Oracle
#[odra::event]
pub struct PriceUpdated {
    pub token_address: Address,
    pub price: i64,
    pub confidence: u64,
    pub exponent: i32,
    pub status: u8,
    pub publisher: Address,
    pub slot: u64,
}

#[odra::event]
pub struct ProductAdded {
    pub product_address: Address,
    pub price_address: Address,
    pub added_by: Address,
}

#[odra::event]
pub struct PublisherAdded {
    pub publisher: Address,
    pub added_by: Address,
}

#[odra::event]
pub struct PublisherRemoved {
    pub publisher: Address,
    pub removed_by: Address,
}

#[odra::event]
pub struct ConfidenceRatioUpdated {
    pub ratio: u64,
    pub updated_by: Address,
}

#[odra::event]
pub struct AdminTransferred {
    pub previous_admin: Address,
    pub new_admin: Address,
}

// Price feed aggregator for multiple oracles
#[odra::module]
pub struct PriceFeedAggregator {
    oracles: List<Address>,
    weights: Mapping<Address, u64>,
    admin: Var<Address>,
}

#[odra::module]
impl PriceFeedAggregator {
    pub fn init(&mut self, admin: Address) {
        self.admin.set(admin);
    }

    pub fn add_oracle(&mut self, oracle_address: Address, weight: u64) {
        let caller = self.env().caller();
        if self.admin.get().unwrap() != caller {
            self.env().revert(LendingError::InvalidOracleConfig);
        }

        // Manual contains check since List doesn't have contains method
        let mut exists = false;
        for addr in self.oracles.iter() {
            if addr == oracle_address {
                exists = true;
                break;
            }
        }

        if !exists {
            self.oracles.push(oracle_address);
        }
        
        self.weights.set(&oracle_address, weight);
    }

    pub fn get_aggregated_price(&self, _token_address: Address, _current_slot: u64) -> Option<u64> {
        let mut total_weight = 0u64;
        let mut weighted_price_sum = 0u64;

        for oracle_addr in self.oracles.iter() {
            if let Some(weight) = self.weights.get(&oracle_addr) {
                // For now, skip cross-contract calls to avoid compilation issues
                // In a real implementation, you would use the correct Odra 2.4 call_contract syntax
                // let price = self.get_price_from_oracle(oracle_addr, token_address, current_slot)?;
                
                // Temporary: use a mock price for compilation
                let price = 100u64; // Mock price
                
                if let Some(weighted_price) = price.checked_mul(weight) {
                    if let Some(new_sum) = weighted_price_sum.checked_add(weighted_price) {
                        weighted_price_sum = new_sum;
                        total_weight = total_weight.checked_add(weight)?;
                    }
                }
            }
        }

        if total_weight == 0 {
            return None;
        }

        weighted_price_sum.checked_div(total_weight)
    }

    // Helper method to get price from oracle (to be implemented with proper cross-contract calls)
    fn get_price_from_oracle(&self, _oracle_addr: Address, _token_address: Address, _current_slot: u64) -> Option<u64> {
        // This is a placeholder for the actual cross-contract call
        // The exact syntax depends on your Odra 2.4 setup
        // You might need to use a different approach for cross-contract calls
        
        // For now, return a mock price
        Some(100u64)
    }
}