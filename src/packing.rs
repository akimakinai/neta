use bevy::{
    math::{Mat2, Vec2},
    prelude::Deref,
};

#[derive(Deref, Clone)]
pub struct EdgeVectors(pub Vec<Vec2>);

impl EdgeVectors {
    pub fn with_rect_size_rotation(size: Vec2, rotation: f32) -> Self {
        let rotation_matrix = Mat2::from_angle(rotation);

        let mut points = Vec::new();
        points.push(rotation_matrix * Vec2::new(0.0, -size.y));
        points.push(rotation_matrix * Vec2::new(size.x, 0.0));
        points.push(rotation_matrix * Vec2::new(0.0, size.y));
        points.push(rotation_matrix * Vec2::new(-size.x, 0.0));

        EdgeVectors(points)
    }

    pub fn neg(&self) -> Self {
        let mut negated = self.0.clone();
        for point in negated.iter_mut() {
            *point = -(*point);
        }
        EdgeVectors(negated)
    }

    fn local_vertices(&self) -> Vec<Vec2> {
        core::iter::once(Vec2::ZERO)
            .chain(self.0.iter().scan(Vec2::ZERO, |acc, edge| {
                *acc += *edge;
                Some(*acc)
            }))
            .collect::<Vec<_>>()
    }
}

fn minkowski_sum(a: &EdgeVectors, b: &EdgeVectors) -> EdgeVectors {
    // https://cp-algorithms.com/geometry/minkowski.html

    // Get the index of bottom-left most point
    fn get_first_index(v: &EdgeVectors) -> usize {
        v.iter()
            .enumerate()
            .min_by(|(_, v), (_, w)| {
                (v.y, v.x)
                    .partial_cmp(&(w.y, w.x))
                    .expect("NaN element in EdgeVectors")
            })
            .unwrap()
            .0
    }

    let mut i = get_first_index(a);
    let mut j = get_first_index(b);

    let mut result = Vec::new();

    for _ in 0..(a.len() + b.len()) {
        let a_i = a[i];
        let b_j = b[j];

        let point = a_i + b_j;
        result.push(point);

        let cross = a_i.perp_dot(b_j);

        if cross > 0.0 {
            i = (i + 1) % a.len();
        } else if cross < 0.0 {
            j = (j + 1) % b.len();
        } else {
            i = (i + 1) % a.len();
            j = (j + 1) % b.len();
        }
    }

    EdgeVectors(result)
}

#[derive(Clone)]
pub struct ShapePosition {
    pub translation: Vec2,
    pub edges: EdgeVectors,
}

impl ShapePosition {
    pub fn vertices(&self) -> Vec<Vec2> {
        // local vertices
        let mut vertices = self.edges.local_vertices();

        let centroid = vertices.iter().fold(Vec2::ZERO, |acc, v| acc + *v) / vertices.len() as f32;

        // translate to world space
        vertices.iter_mut().for_each(|v| {
            *v += self.translation;
            *v -= centroid;
        });

        vertices
    }
}

fn fill(placed_shapes: &[ShapePosition], shape_to_place: &ShapePosition) -> ShapePosition {
    for placed in placed_shapes {
        let placed_vertices = placed.vertices();

        let nfp = minkowski_sum(&placed.edges, &shape_to_place.edges.neg());

        let mut nfp_vertices = ShapePosition {
            translation: placed.translation,
            edges: nfp,
        }
        .vertices();

        // Sort by distance to initial translation of `shape_to_place`
        nfp_vertices.sort_by(|v, w| {
            (v - shape_to_place.translation)
                .length()
                .partial_cmp(&(w - shape_to_place.translation).length())
                .expect("NaN")
        });

        for nfp_vertex in nfp_vertices {
            let translated_shape = ShapePosition {
                translation: nfp_vertex,
                edges: shape_to_place.edges.clone(),
            };

            let mut inside = false;
            for edge in translated_shape.vertices() {
                if is_inside(&placed_vertices, edge) {
                    inside = true;
                    break;
                }
            }
            if !inside {
                return translated_shape;
            }
        }
    }

    return shape_to_place.clone();
}

fn is_inside(polygon: &Vec<Vec2>, point: Vec2) -> bool {
    let mut is_cross_positive = None;

    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];

        let cross = (b - a).perp_dot(point - a);

        if let Some(is_cross_positive) = is_cross_positive {
            if is_cross_positive != (cross > 0.0) {
                return false;
            }
        } else {
            is_cross_positive = Some(cross > 0.0);
        }
    }

    return true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_inside() {
        let polygon = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 4.0),
        ];

        assert!(is_inside(&polygon, Vec2::new(2.0, 2.0)));
        assert!(!is_inside(&polygon, Vec2::new(5.0, 5.0)));
        assert!(!is_inside(&polygon, Vec2::new(2.0, 5.0)));
        assert!(!is_inside(&polygon, Vec2::new(-2.0, 2.0)));
    }

    #[test]
    fn test_fill() {
        let placed_shapes = vec![ShapePosition {
            translation: Vec2::new(0.0, 0.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 4.0), 0.0),
        }];

        let shape_to_place = ShapePosition {
            translation: Vec2::new(25.0, 25.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(2.0, 2.0), 0.0),
        };

        let result = fill(&placed_shapes, &shape_to_place);

        // Ensure the result is not overlapping with the placed shape
        for placed in &placed_shapes {
            for vertex in result.vertices() {
                assert!(!is_inside(&placed.vertices(), vertex));
            }
        }

        assert!(result.translation.length() < 5.0);
    }
}
