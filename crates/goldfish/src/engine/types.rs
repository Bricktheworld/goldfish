#[derive(Debug, Copy, Clone)]
pub struct Size {
	pub width: u32,
	pub height: u32,
}

impl Size {
	pub fn aspect(&self) -> f64 {
		self.width as f64 / self.height as f64
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Color {
	pub r: f32,
	pub g: f32,
	pub b: f32,
	pub a: f32,
}

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Vec2")]
pub struct Vec2Serde {
	x: f32,
	y: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Vec3")]
pub struct Vec3Serde {
	x: f32,
	y: f32,
	z: f32,
}
