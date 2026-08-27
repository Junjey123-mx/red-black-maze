mod player;
mod weapon;

pub use player::Player;
pub(crate) use weapon::{Weapon, WeaponState};
// `WeaponTier` se re-exporta en el Commit 13, cuando `game::session`
// pasa a consumirlo para el daño por tier.
