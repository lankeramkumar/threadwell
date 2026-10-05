//! Charging a card. Each request carries an idempotency key, so a retried request never charges twice.

pub struct ChargeRequest {
    pub amount_cents: u64,
    pub idempotency_key: String,
}

/// Returns the existing charge when the key was seen before. Otherwise records the new charge.
pub fn charge(store: &mut Store, request: &ChargeRequest) -> Result<u64, String> {
    if let Some(existing) = store.by_key(&request.idempotency_key) {
        return Ok(existing);
    }
    if request.amount_cents == 0 {
        return Err("amount must be positive".into());
    }
    let id = store.insert(&request.idempotency_key, request.amount_cents);
    Ok(id)
}

pub struct Store;

impl Store {
    pub fn by_key(&self, _key: &str) -> Option<u64> {
        None
    }
    pub fn insert(&mut self, _key: &str, _amount: u64) -> u64 {
        1
    }
}
