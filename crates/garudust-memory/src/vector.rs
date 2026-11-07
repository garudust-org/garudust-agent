use anyhow::{bail, Result};

/// Encode a slice of 32-bit floats into little-endian bytes.
#[must_use]
pub fn f32_slice_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

/// Decode little-endian bytes back into a vector of 32-bit floats.
///
/// # Errors
///
/// Returns an error if the input slice length is not a multiple of 4.
pub fn bytes_to_f32_vec(b: &[u8]) -> Result<Vec<f32>> {
    if !b.len().is_multiple_of(4) {
        bail!("byte slice length ({}) must be a multiple of 4", b.len());
    }
    let count = b.len() / 4;
    let mut vec = Vec::with_capacity(count);
    for chunk in b.chunks_exact(4) {
        let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        vec.push(f32::from_le_bytes(arr));
    }
    Ok(vec)
}

/// Calculate the Euclidean (L2) norm of a vector.
#[must_use]
pub fn l2_norm(v: &[f32]) -> f32 {
    let sum_sq: f32 = v.iter().map(|&x| x * x).sum();
    sum_sq.sqrt()
}

/// Normalize vector in-place to have Euclidean (L2) norm of 1.0.
/// If vector is degenerate (near zero norm), it remains unchanged.
pub fn normalize_l2(v: &mut [f32]) {
    let norm = l2_norm(v);
    if norm > 1e-10 {
        let inv = 1.0 / norm;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
}

/// Compute the dot product between two float vectors.
/// Returns 0.0 if the lengths differ.
#[must_use]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
}

/// Compute the cosine similarity between two vectors.
/// Result is clamped to `[-1.0, 1.0]`.
/// Returns 0.0 if lengths differ or either vector has near-zero magnitude.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);
    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }
    let dot = dot_product(a, b);
    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_roundtrip() {
        let original = vec![1.0_f32, -2.5, 0.0, std::f32::consts::PI, -100.25];
        let bytes = f32_slice_to_bytes(&original);
        assert_eq!(bytes.len(), original.len() * 4);
        let decoded = bytes_to_f32_vec(&bytes).expect("decode failed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_invalid_bytes_length() {
        let bytes = vec![0u8; 7]; // Not multiple of 4
        assert!(bytes_to_f32_vec(&bytes).is_err());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-5);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) + 1.0).abs() < 1e-5);

        let diff_len = vec![1.0, 0.0];
        assert!(cosine_similarity(&a, &diff_len).abs() < 1e-5);

        let zeros = vec![0.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &zeros).abs() < 1e-5);
    }

    #[test]
    fn test_normalize_l2() {
        let mut v = vec![3.0, 4.0];
        normalize_l2(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-5);
        assert!((v[1] - 0.8).abs() < 1e-5);
        assert!((l2_norm(&v) - 1.0).abs() < 1e-5);
    }
}
