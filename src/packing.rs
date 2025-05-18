use bevy::{
    math::{Mat2, Vec2},
    prelude::Deref,
};

/// A polygon represented by its edge vectors (CCW order).
#[derive(Deref, Clone, Debug)]
pub struct EdgeVectors(pub Vec<Vec2>);

impl EdgeVectors {
    pub fn with_rect_size_rotation(size: Vec2, rotation: f32) -> Self {
        let rotation_matrix = Mat2::from_angle(rotation);

        // Ordered so that the first point is the bottom-left most
        let points = vec![
            rotation_matrix * Vec2::new(size.x, 0.0),
            rotation_matrix * Vec2::new(0.0, size.y),
            rotation_matrix * Vec2::new(-size.x, 0.0),
            rotation_matrix * Vec2::new(0.0, -size.y),
        ];

        EdgeVectors(points)
    }

    /// Construct from a list of vertices.
    pub fn from_vertices(vertices: &[Vec2]) -> Self {
        let mut edges = Vec::with_capacity(vertices.len());
        for i in 0..vertices.len() {
            let a = vertices[i];
            let b = vertices[(i + 1) % vertices.len()];
            edges.push(b - a);
        }
        EdgeVectors(edges)
    }

    /// Return the iterator of vertices. The initial vertex is (0, 0).
    fn local_vertices(&self) -> impl Iterator<Item = Vec2> {
        self.0.iter().scan(Vec2::ZERO, |acc, edge| {
            *acc += *edge;
            Some(*acc)
        })
    }

    /// Dvide each edge vector into `n` segments.
    fn divide(&self, n: u32) -> EdgeVectors {
        let mut edges = Vec::with_capacity(self.len() * 2);
        for a in &self.0 {
            let ad = *a / n as f32;
            for _ in 0..n {
                edges.push(ad);
            }
        }
        EdgeVectors(edges)
    }
}

/// Compute the Minkowski sum of two polygons.
fn minkowski_sum(a: &EdgeVectors, b: &EdgeVectors) -> EdgeVectors {
    // https://cp-algorithms.com/geometry/minkowski.html

    // Get the index of bottom-left most point
    fn get_first_index(v: impl Iterator<Item = Vec2>) -> usize {
        v.into_iter()
            .enumerate()
            .min_by(|(_, v), (_, w)| {
                (v.y, v.x)
                    .partial_cmp(&(w.y, w.x))
                    .expect("NaN element in EdgeVectors")
            })
            .unwrap()
            .0
    }

    let mut i = get_first_index(a.local_vertices());
    let mut j = get_first_index(b.local_vertices());

    let mut i_inc = 0;
    let mut j_inc = 0;

    let mut result = Vec::with_capacity(a.len().max(b.len()));

    let mut cur = a[i] + b[j];

    // Iterate until we have traversed all edges of both shapes
    while i_inc < a.len() || j_inc < b.len() {
        let a_i = a[i];
        let b_j = b[j];

        result.push(cur);

        let cross = a_i.perp_dot(b_j);

        if cross >= 0.0 && i_inc < a.len() {
            i = (i + 1) % a.len();
            i_inc += 1;
            cur += a[i];
        }
        if cross <= 0.0 && j_inc < b.len() {
            j = (j + 1) % b.len();
            j_inc += 1;
            cur += b[j];
        }
    }

    EdgeVectors::from_vertices(&result)
}

#[derive(Clone, Debug)]
pub struct ShapePosition {
    pub translation: Vec2,
    pub edges: EdgeVectors,
}

impl ShapePosition {
    pub fn vertices(&self) -> Vec<Vec2> {
        // local vertices
        let mut vertices = self.edges.local_vertices().collect::<Vec<_>>();

        let centroid = calculate_centroid(&vertices);

        // translate to world space
        vertices.iter_mut().for_each(|v| {
            *v += self.translation;
            *v -= centroid;
        });

        vertices
    }

    fn offset(&mut self, width: f32) {
        let vertices = self.vertices();

        let mut new_vertices = vertices.clone();

        for i in 0..self.edges.len() {
            // Outward normal
            let prev_normal = -self.edges[i].perp().normalize();
            let next_normal = -self.edges[(i + 1) % self.edges.len()].perp().normalize();

            new_vertices[i] = vertices[i] + (prev_normal + next_normal) * width;
        }

        self.edges = EdgeVectors::from_vertices(&new_vertices);
        self.translation -= calculate_centroid(&new_vertices) - calculate_centroid(&vertices);
    }

    fn is_overlapping(&self, other: &ShapePosition) -> bool {
        // Check overlap using the Separating Axis Theorem (SAT)

        let normals = (self.edges.0.iter().map(|v| v.perp()))
            .chain(other.edges.0.iter().map(|v| v.perp()))
            .collect::<Vec<_>>();

        for normal in normals {
            let mut min_a = f32::MAX;
            let mut max_a = f32::MIN;

            for vertex in self.vertices() {
                let projection = vertex.dot(normal);
                min_a = min_a.min(projection);
                max_a = max_a.max(projection);
            }

            let mut min_b = f32::MAX;
            let mut max_b = f32::MIN;

            for vertex in other.vertices() {
                let projection = vertex.dot(normal);
                min_b = min_b.min(projection);
                max_b = max_b.max(projection);
            }

            if max_a < min_b || max_b < min_a {
                return false;
            }
        }

        true
    }
}

pub fn fill<'a>(
    placed_shapes: impl IntoIterator<Item = &'a ShapePosition> + Clone,
    shape_to_place: &ShapePosition,
    offset: f32,
    div: Option<u32>,
) -> ShapePosition {
    let mut candidates = vec![];

    for placed in placed_shapes.clone() {
        let nfp = minkowski_sum(&placed.edges, &shape_to_place.edges);
        // debug_draw_vertices(placed.vertices());
        // debug_draw_vertices(shape_to_place.vertices());

        let mut nfp_shape = ShapePosition {
            translation: placed.translation,
            edges: nfp.divide(div.unwrap_or(1)),
        };
        nfp_shape.offset(offset);
        let mut nfp_vertices = nfp_shape.vertices();
        // debug_draw_vertices(nfp_vertices.clone());

        // Sort by distance to initial translation of `shape_to_place`
        nfp_vertices.sort_by(|v, w| {
            (v - shape_to_place.translation)
                .length()
                .partial_cmp(&(w - shape_to_place.translation).length())
                .expect("NaN")
        });

        for nfp_vertex in nfp_vertices {
            // A vertex on the Minkowski sum is a candidate for the new shape position.
            // If the translated shape is not inside any of the placed shapes, add it to candidates.

            let mut inside = false;

            let translated = ShapePosition {
                translation: nfp_vertex,
                edges: shape_to_place.edges.clone(),
            };

            // TODO: use a spatial partitioning to speed this up
            for placed2 in placed_shapes.clone() {
                let mut placed2 = placed2.clone();
                placed2.offset(offset);
                if translated.is_overlapping(&placed2) {
                    inside = true;
                    break;
                }
            }

            if !inside {
                let translated_shape = ShapePosition {
                    translation: nfp_vertex,
                    edges: shape_to_place.edges.clone(),
                };
                candidates.push(translated_shape);
            }
        }
    }

    // Needs heuristic (like, choosing from 4 directions based on original translation)
    // For now, choose one that is nearest to original translation
    candidates.sort_by(|a, b| {
        (a.translation - shape_to_place.translation)
            .length()
            .partial_cmp(&(b.translation - shape_to_place.translation).length())
            .expect("NaN")
    });

    candidates.swap_remove(0)
}

fn calculate_centroid(vertices: &[Vec2]) -> Vec2 {
    let mut centroid = Vec2::ZERO;
    for vertex in vertices {
        centroid += *vertex;
    }
    centroid / vertices.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_overlapping() {
        let a = ShapePosition {
            translation: Vec2::new(0.0, 0.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 4.0), 0.0),
        };

        let b = ShapePosition {
            translation: Vec2::new(2.0, 2.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 4.0), 0.0),
        };

        let c = ShapePosition {
            translation: Vec2::new(0.0, 5.5),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 4.0), 0.0),
        };

        assert!(a.is_overlapping(&b));
        assert!(!a.is_overlapping(&c));
        assert!(b.is_overlapping(&c));
    }

    #[test]
    fn test_minkowski_sum() {
        let a = EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 3.0), 0.0);
        let b = EdgeVectors::with_rect_size_rotation(Vec2::new(2.0, 1.0), 0.0);

        let result = minkowski_sum(&a, &b);

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Vec2::new(6.0, 0.0));
        assert_eq!(result[1], Vec2::new(0.0, 4.0));
        assert_eq!(result[2], Vec2::new(-6.0, 0.0));
        assert_eq!(result[3], Vec2::new(0.0, -4.0));

        let a = EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 3.0), 0.0);
        let b = EdgeVectors::with_rect_size_rotation(Vec2::new(2.0, 1.0), 0.0);

        let result = minkowski_sum(&a, &b).divide(2);

        assert_eq!(result.len(), 8);
        assert_eq!(result[0], Vec2::new(3.0, 0.0));
        assert_eq!(result[1], Vec2::new(3.0, 0.0));
        assert_eq!(result[2], Vec2::new(0.0, 2.0));
        assert_eq!(result[3], Vec2::new(0.0, 2.0));
        assert_eq!(result[4], Vec2::new(-3.0, 0.0));
        assert_eq!(result[5], Vec2::new(-3.0, 0.0));
        assert_eq!(result[6], Vec2::new(0.0, -2.0));
        assert_eq!(result[7], Vec2::new(0.0, -2.0));
    }

    #[test]
    fn test_fill() {
        let mut placed_shapes = vec![ShapePosition {
            translation: Vec2::new(0.0, 0.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 4.0), 0.0),
        }];

        let shape_to_place = ShapePosition {
            translation: Vec2::new(25.0, 25.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(2.0, 2.0), 0.0),
        };

        let result = fill(&placed_shapes, &shape_to_place, 0.1, Some(2));

        // Ensure the result is not overlapping with the placed shape
        for placed in &placed_shapes {
            assert!(!result.is_overlapping(placed));
        }

        let gap = 0.1;

        assert!(
            (result.translation.length() - 3.0 * std::f32::consts::SQRT_2 - gap) < 0.1,
            "{:?}",
            result.translation
        );

        // Since we initially placed the shape at (25, 25), the result should be close to that
        assert!(result.translation.cmpgt(Vec2::new(0.0, 0.0)).all());

        placed_shapes.push(result.clone());

        let shape_to_place2 = ShapePosition {
            translation: Vec2::new(0.0, 0.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(10.0, 10.0), 0.0),
        };

        let result2 = fill(&placed_shapes, &shape_to_place2, 0.1, Some(2));

        for placed in &placed_shapes {
            assert!(!result2.is_overlapping(placed));
        }
    }
}
