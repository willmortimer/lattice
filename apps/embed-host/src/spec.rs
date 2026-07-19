use lattice_embedding::{
    DistanceMetric, EmbeddingSpecification, PoolingStrategy,
};

use crate::EmbeddingSpec;

pub fn embedding_spec_to_proto(spec: &EmbeddingSpecification) -> EmbeddingSpec {
    EmbeddingSpec {
        provider_id: spec.provider_id.clone(),
        model_id: spec.model_id.clone(),
        model_revision: spec.model_revision.clone(),
        artifact_sha256: spec.artifact_sha256.clone(),
        dimensions: spec.dimensions,
        native_dimensions: spec.native_dimensions,
        distance: distance_to_wire(spec.distance),
        pooling: pooling_to_wire(spec.pooling),
        normalized: spec.normalized,
        instruction_version: spec.instruction_version.clone(),
    }
}

pub fn embedding_spec_from_proto(spec: &EmbeddingSpec) -> Result<EmbeddingSpecification, String> {
    Ok(EmbeddingSpecification {
        provider_id: spec.provider_id.clone(),
        model_id: spec.model_id.clone(),
        model_revision: spec.model_revision.clone(),
        artifact_sha256: spec.artifact_sha256.clone(),
        dimensions: spec.dimensions,
        native_dimensions: spec.native_dimensions,
        distance: distance_from_wire(&spec.distance)?,
        pooling: pooling_from_wire(&spec.pooling)?,
        normalized: spec.normalized,
        instruction_version: spec.instruction_version.clone(),
    })
}

fn distance_to_wire(distance: DistanceMetric) -> String {
    match distance {
        DistanceMetric::Cosine => "cosine".into(),
        DistanceMetric::Dot => "dot".into(),
        DistanceMetric::L2 => "l2".into(),
    }
}

fn pooling_to_wire(pooling: PoolingStrategy) -> String {
    match pooling {
        PoolingStrategy::Last => "last".into(),
        PoolingStrategy::Mean => "mean".into(),
        PoolingStrategy::Cls => "cls".into(),
    }
}

fn distance_from_wire(value: &str) -> Result<DistanceMetric, String> {
    match value {
        "cosine" => Ok(DistanceMetric::Cosine),
        "dot" => Ok(DistanceMetric::Dot),
        "l2" => Ok(DistanceMetric::L2),
        other => Err(format!("unknown distance metric: {other}")),
    }
}

fn pooling_from_wire(value: &str) -> Result<PoolingStrategy, String> {
    match value {
        "last" => Ok(PoolingStrategy::Last),
        "mean" => Ok(PoolingStrategy::Mean),
        "cls" => Ok(PoolingStrategy::Cls),
        other => Err(format!("unknown pooling strategy: {other}")),
    }
}
