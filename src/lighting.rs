use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Off,
    #[default]
    Solid,
    Snake,
    Rainbow,
    Breath,
    Gradient,
    ShallowBreath,
}
impl Effect {
    pub const ALL: [Self; 7] = [
        Self::Off,
        Self::Solid,
        Self::Snake,
        Self::Rainbow,
        Self::Breath,
        Self::Gradient,
        Self::ShallowBreath,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Solid => "Solid",
            Self::Snake => "Snake",
            Self::Rainbow => "Rainbow",
            Self::Breath => "Breathing",
            Self::Gradient => "Gradient",
            Self::ShallowBreath => "Gentle breathing",
        }
    }
    pub fn animated(self) -> bool {
        !matches!(self, Self::Off | Self::Solid)
    }
    fn code(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Solid => 1,
            Self::Snake => 2,
            Self::Rainbow => 3,
            Self::Breath => 4,
            Self::Gradient => 5,
            Self::ShallowBreath => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Light {
    pub color: u32,
    pub effect: Effect,
    pub brightness: u8,
    pub speed: u8,
}
impl Default for Light {
    fn default() -> Self {
        Self {
            color: 0x81a1c1,
            effect: Effect::Solid,
            brightness: 40,
            speed: 40,
        }
    }
}
impl Light {
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.color <= 0xffffff,
            "Lighting color must be a 24-bit RGB value"
        );
        ensure!(
            self.brightness <= 100 && self.speed <= 100,
            "Lighting brightness and speed must be between 0 and 100"
        );
        Ok(())
    }
    fn params(self) -> Value {
        json!({"c":self.color,"e":self.effect.code(),"b":f64::from(self.brightness)/100.,"s":f64::from(self.speed)/100.})
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Lighting {
    pub agents: [Light; 6],
    pub keys: Light,
    pub ambient: Light,
}
impl Lighting {
    pub fn validate(&self) -> Result<()> {
        for light in self.agents.iter().chain([&self.keys, &self.ambient]) {
            light.validate()?;
        }
        Ok(())
    }
    pub fn light(&self, target: usize) -> &Light {
        match target {
            0..=5 => &self.agents[target],
            6 => &self.keys,
            7 => &self.ambient,
            _ => unreachable!("Lighting target"),
        }
    }
    pub fn light_mut(&mut self, target: usize) -> &mut Light {
        match target {
            0..=5 => &mut self.agents[target],
            6 => &mut self.keys,
            7 => &mut self.ambient,
            _ => unreachable!("Lighting target"),
        }
    }
    pub fn requests(&self, id: &mut u64) -> [Value; 2] {
        let mut keys = self.keys.params();
        keys["m"] = json!(0);
        let mut ambient = self.ambient.params();
        ambient["m"] = json!(0);
        *id += 1;
        let zones =
            json!({"id":*id,"method":"v.oai.rgbcfg","params":{"keys":keys,"ambient":ambient}});
        let agents: Vec<_> = self
            .agents
            .iter()
            .enumerate()
            .map(|(index, light)| {
                let mut params = light.params();
                params["id"] = json!(index);
                params["sk"] = json!(0);
                params["sa"] = json!(0);
                params
            })
            .collect();
        *id += 1;
        [
            zones,
            json!({"id":*id,"method":"v.oai.thstatus","params":agents}),
        ]
    }
}

// A new instance per connection forces saved lighting to be replayed after reconnect.
#[derive(Default)]
pub struct Sync {
    applied: Option<Lighting>,
}
impl Sync {
    pub fn update(&mut self, layer: Option<u64>, lighting: &Lighting, id: &mut u64) -> Vec<Value> {
        if layer != Some(1) {
            self.applied = None;
            return Vec::new();
        }
        if self.applied.as_ref() == Some(lighting) {
            return Vec::new();
        }
        self.applied = Some(lighting.clone());
        lighting.requests(id).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Profiles, config_path},
        protocol::{Decoder, reports},
    };
    #[test]
    fn wire_messages_preserve_all_colors_effects_and_independent_zones() {
        let mut lighting = Lighting::default();
        for (index, effect) in Effect::ALL.into_iter().enumerate() {
            *lighting.light_mut(index) = Light {
                color: 0x123456 + index as u32,
                effect,
                brightness: 75,
                speed: 20,
            };
        }
        lighting.ambient.color = 0xffffff;
        let mut id = 40;
        let requests = lighting.requests(&mut id);
        assert_eq!(requests[0]["params"]["ambient"]["c"], 0xffffff);
        assert_eq!(requests[0]["params"]["keys"]["e"], 6);
        assert_eq!(requests[0]["id"], 41);
        assert_eq!(requests[1]["id"], 42);
        for (index, params) in requests[1]["params"].as_array().unwrap().iter().enumerate() {
            assert_eq!(params["id"], index);
            assert_eq!(params["e"], index);
            assert_eq!(params["b"], 0.75);
            assert_eq!(params["s"], 0.2);
            assert_eq!(params["sk"], 0);
            assert_eq!(params["sa"], 0);
        }
        let mut decoder = Decoder::default();
        let decoded: Vec<_> = requests
            .iter()
            .flat_map(|value| reports(value).unwrap())
            .flat_map(|report| decoder.feed(&report).unwrap())
            .collect();
        assert_eq!(decoded, requests);
    }
    #[test]
    fn sync_is_layer_scoped_and_replays_changes_and_reconnections() {
        let mut sync = Sync::default();
        let mut id = 0;
        let mut lighting = Lighting::default();
        assert!(sync.update(None, &lighting, &mut id).is_empty());
        assert!(sync.update(Some(2), &lighting, &mut id).is_empty());
        assert_eq!(sync.update(Some(1), &lighting, &mut id).len(), 2);
        assert!(sync.update(Some(1), &lighting, &mut id).is_empty());
        lighting.agents[0].color = 0;
        assert_eq!(sync.update(Some(1), &lighting, &mut id).len(), 2);
        assert!(sync.update(Some(0), &lighting, &mut id).is_empty());
        assert_eq!(sync.update(Some(1), &lighting, &mut id).len(), 2);
        assert_eq!(Sync::default().update(Some(1), &lighting, &mut id).len(), 2);
    }
    #[test]
    fn legacy_profiles_have_default_lighting_and_invalid_changes_cannot_replace_saved_config() {
        let mut profiles = Profiles::parse("name = 'Legacy'\n").unwrap();
        assert_eq!(profiles.active().lighting, Lighting::default());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(config_path().file_name().unwrap());
        profiles.save(&path).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            Profiles::load(&path).unwrap().active().lighting,
            profiles.active().lighting
        );
        for invalid in [
            Light {
                color: 0x1000000,
                ..Light::default()
            },
            Light {
                brightness: 101,
                ..Light::default()
            },
            Light {
                speed: 101,
                ..Light::default()
            },
        ] {
            profiles.active_mut().lighting.keys = invalid;
            assert!(profiles.save(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        }
    }
}
