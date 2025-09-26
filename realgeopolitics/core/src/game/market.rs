use rand::Rng;
use rand::rngs::StdRng;

#[derive(Debug, Clone)]
pub struct CommodityMarket {
    price: f64,
    base_price: f64,
    volatility: f64,
    shock_chance: f64,
}

impl CommodityMarket {
    pub fn new(base_price: f64, volatility: f64, shock_chance: f64) -> Self {
        Self {
            price: base_price.max(1.0),
            base_price: base_price.max(1.0),
            volatility: volatility.max(0.1),
            shock_chance: shock_chance.clamp(0.0, 1.0),
        }
    }

    pub fn price(&self) -> f64 {
        self.price
    }

    pub fn update(&mut self, rng: &mut StdRng, scale: f64) -> Option<String> {
        let adjusted_scale = scale.max(0.25);
        let drift = (self.base_price - self.price) * 0.02 * adjusted_scale;
        let random_step = rng.gen_range(-self.volatility..self.volatility) * adjusted_scale.sqrt();
        let mut new_price = self.price + drift + random_step;
        let shock_triggered = rng.gen_bool((self.shock_chance * adjusted_scale).clamp(0.0, 1.0));
        let mut message = None;
        if shock_triggered {
            let shock_multiplier = if rng.gen_bool(0.5) { 1.35 } else { 0.7 };
            new_price *= shock_multiplier;
            message = Some(if shock_multiplier > 1.0 {
                format!(
                    "資源市場に価格高騰ショックが発生しました (倍率 x{:.2})",
                    shock_multiplier
                )
            } else {
                format!(
                    "資源市場で価格急落イベントが発生しました (倍率 x{:.2})",
                    shock_multiplier
                )
            });
        }

        self.price = new_price.clamp(self.base_price * 0.4, self.base_price * 1.9);
        message
    }

    pub fn revenue_for(&self, resource_index: i32, scale: f64) -> f64 {
        let resources = resource_index.max(0) as f64;
        let export_volume = resources * 0.45;
        (self.price * export_volume * scale).max(0.0)
    }
}
