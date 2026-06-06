use raylib::prelude::Vector2;

// Scale vector (multiply by value)
pub fn vector2_scale(v: Vector2, scale: f32) -> Vector2 {
    Vector2 {
        x: v.x * scale,
        y: v.y * scale,
    }
}

// Subtract two vectors (v1 - v2)
pub fn vector2_subtract(v1: Vector2, v2: Vector2) -> Vector2 {
    Vector2 {
        x: v1.x - v2.x,
        y: v1.y - v2.y,
    }
}

// Add two vectors (v1 + v2)
pub fn vector2_add(v1: Vector2, v2: Vector2) -> Vector2 {
    Vector2 {
        x: v1.x + v2.x,
        y: v1.y + v2.y,
    }
}
