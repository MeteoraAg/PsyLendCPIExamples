// Various PsyLend structs use a cache to store data on-chain. The cache is valid for some TTL,
// expressed in slots, which varies by the data being stored.
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Cache<T, const TTL: u64> {
    /// The value being cached
    value: T,

    /// The last slot that this information was updated in
    last_updated: u64,

    /// Whether the value has been manually invalidated
    invalidated: u8,

    _reserved: [u8; 7],
}

// Since the `Cache` type uses generic parameters we can't use the derive macros
// which help with some validation. The fields used are all pod-safe types, so
// a `Cache` should also be pod-safe if the value stored within it is.
unsafe impl<T, const TTL: u64> Pod for Cache<T, TTL> where T: Pod {}
unsafe impl<T, const TTL: u64> Zeroable for Cache<T, TTL> where T: Zeroable {}

/// Store calculated values that can be manually invalidated or expire after some number of slots
/// Methods expect a "current_slot" argument which should indicate which slot the calculation is
/// relevant for. This is usually the actual current slot but may be an older slot if the value is
/// used or calculated for a previous slot, for example a partial refresh of the reserve.
impl<T, const TTL: u64> Cache<T, TTL> {
    pub fn new(value: T, current_slot: u64) -> Self {
        Self {
            value,
            invalidated: 0,
            last_updated: current_slot,
            _reserved: [0; 7],
        }
    }

    pub fn validate_fresh(&self, current_slot: u64) -> Result<(), CacheInvalidError> {
        if current_slot < self.last_updated {
            return Err(CacheInvalidError::TooNew {
                msg: self.time_msg(current_slot),
            });
        }
        if current_slot - self.last_updated > TTL {
            return Err(CacheInvalidError::Expired {
                msg: self.time_msg(current_slot),
            });
        }
        if self.invalidated != 0 {
            return Err(CacheInvalidError::Invalidated);
        }
        Ok(())
    }

    fn time_msg(&self, current_slot: u64) -> String {
        format!(
            "last_updated = {}, time_to_live = {}, current_slot = {}",
            self.last_updated, TTL, current_slot
        )
    }

    /// If the cache is neither expired nor marked invalid, return the value,
    /// otherwise return an error indicating why it is stale
    pub fn try_get(&self, current_slot: u64) -> Result<&T, CacheInvalidError> {
        self.validate_fresh(current_slot)?;
        Ok(&self.value)
    }

    /// If the cache is neither expired nor marked invalid, return the value mutably,
    /// otherwise return an error indicating why it is stale
    pub fn try_get_mut(&mut self, current_slot: u64) -> Result<&mut T, CacheInvalidError> {
        self.validate_fresh(current_slot)?;
        Ok(&mut self.value)
    }

    /// If the cache is neither expired nor marked invalid, return the value,
    /// otherwise panic with an error message describing the item and why it is stale
    pub fn expect(&self, current_slot: u64, description: &str) -> &T {
        self.try_get(current_slot).expect(description)
    }

    /// If the cache is neither expired nor marked invalid, return the value mutably,
    /// otherwise panic with an error message describing the item and why it is stale
    pub fn expect_mut(&mut self, current_slot: u64, description: &str) -> &mut T {
        self.try_get_mut(current_slot).expect(description)
    }

    /// Returns the current value, regardless of whether or not it is stale
    pub fn get_stale(&self) -> &T {
        &self.value
    }

    /// Returns the current value mutably, regardless of whether or not it is stale.
    pub fn get_stale_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Replace the current value and reset the state to the current slot
    pub fn refresh_as(&mut self, value: T, current_slot: u64) {
        self.value = value;
        self.invalidated = 0;
        self.last_updated = current_slot;
    }

    /// Marks the data as stale
    pub fn invalidate(&mut self) {
        self.invalidated = 1;
    }

    /// Returns the slot when this data was last updated
    pub fn last_updated(&self) -> u64 {
        self.last_updated
    }

    // TODO this is identical to `refresh_to` and the comment is incorrect (it is mutating)
    /// Updates the cache to be valid and at current_slot without mutating the value.
    pub fn refresh(&mut self, current_slot: u64) {
        self.invalidated = 0;
        self.last_updated = current_slot;
    }

    /// Updates the cache to be valid and increments the last updated slot
    pub fn refresh_additional(&mut self, additional_slots: u64) {
        self.invalidated = 0;
        self.last_updated += additional_slots;
    }

    /// Updates the cache to be valid and sets the last updated slot
    pub fn refresh_to(&mut self, current_slot: u64) {
        self.invalidated = 0;
        self.last_updated = current_slot;
    }
}

#[derive(Debug)]
pub enum CacheInvalidError {
    /// The cache is too old to use for the current slot.
    Expired { msg: String },

    /// A calculation was attempted for a slot that is too old to use the cache,
    /// since the cache was created more recently than the relevant slot.
    TooNew { msg: String },

    /// The cache has been manually invalidated and may no longer be used.
    Invalidated,
}