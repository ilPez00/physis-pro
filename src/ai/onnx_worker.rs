use crate::graph::NodePayload;
use crossbeam_channel::{unbounded, Sender};
use std::thread;

pub enum OnnxRequest {
    IngestFilter {
        signal_type: u8,
        raw_buffer: Vec<u8>,
        reply_to: Sender<OnnxResponse>,
    },
    TopologicalRoute {
        node_id: u64,
        embedding: Vec<f32>,
        neighborhood_embeddings: Vec<(u64, Vec<f32>)>,
        reply_to: Sender<OnnxResponse>,
    },
    GnnContext {
        nodes: Vec<(u64, Vec<f32>, NodePayload)>,
        edges: Vec<crate::graph::Edge>,
        reply_to: Sender<OnnxResponse>,
    },
}

pub enum OnnxResponse {
    IngestResult {
        cleaned_text: String,
        payload: NodePayload,
    },
    RouteResult {
        predicted_edges: Vec<(u64, f32)>, // (target_id, confidence)
    },
    GnnResult {
        context_vector: Vec<f32>,
    },
}

pub struct OnnxHolon {
    tx: Sender<OnnxRequest>,
}

impl OnnxHolon {
    pub fn spawn() -> Self {
        let (tx, rx) = unbounded::<OnnxRequest>();

        thread::spawn(move || {
            // Here we would initialize the ONNX Runtime sessions
            // let session = ort::Session::builder()...

            while let Ok(request) = rx.recv() {
                match request {
                    OnnxRequest::IngestFilter {
                        signal_type,
                        raw_buffer,
                        reply_to,
                    } => {
                        // Phase 1: Signal-specific activation
                        let cleaned_text = String::from_utf8_lossy(&raw_buffer).to_string();

                        let activation = match signal_type {
                            3 => 2.0, // ThoughtCapture: High priority
                            4 => 1.5, // VisionFeature: Medium-High
                            0 => 1.2, // AudioPCM: Medium
                            _ => 1.0, // Defaults
                        };

                        let payload = NodePayload {
                            activation_energy: activation,
                            decay_metrics: 0.0,
                            prune_flag: 0,
                            compress_flag: 0,
                            _padding: [0; 2],
                        };
                        let _ = reply_to.send(OnnxResponse::IngestResult {
                            cleaned_text,
                            payload,
                        });
                    }
                    OnnxRequest::TopologicalRoute {
                        node_id: _,
                        embedding,
                        neighborhood_embeddings,
                        reply_to,
                    } => {
                        // Phase 2: Predict edges based on vector similarity
                        let mut predicted_edges = Vec::new();
                        let threshold = 0.85;

                        for (neighbor_id, n_emb) in neighborhood_embeddings {
                            let sim = crate::models::cosine_sim(&embedding, &n_emb);
                            if sim > threshold {
                                predicted_edges.push((neighbor_id, sim));
                            }
                        }

                        let _ = reply_to.send(OnnxResponse::RouteResult { predicted_edges });
                    }
                    OnnxRequest::GnnContext {
                        nodes,
                        edges,
                        reply_to,
                    } => {
                        // Refined Phase 3: Attention-Weighted Pooling
                        // Aggregates based on node activation energy and edge connectivity
                        if nodes.is_empty() {
                            let _ = reply_to.send(OnnxResponse::GnnResult {
                                context_vector: vec![],
                            });
                            continue;
                        }

                        let dim = nodes[0].1.len();
                        let mut context_vector = vec![0.0f32; dim];
                        let mut total_weight = 0.0f32;

                        for (id, emb, payload) in &nodes {
                            // Weight = Node Activation * (1 + sum of incident edge weights)
                            let mut connectivity = 0.0f32;
                            for edge in &edges {
                                if edge.source.data == *id || edge.target.data == *id {
                                    connectivity += edge.weight;
                                }
                            }

                            let node_weight = payload.activation_energy * (1.0 + connectivity);
                            total_weight += node_weight;

                            for (i, val) in emb.iter().enumerate() {
                                context_vector[i] += val * node_weight;
                            }
                        }

                        if total_weight > 0.0 {
                            for val in &mut context_vector {
                                *val /= total_weight;
                            }
                        }

                        let _ = reply_to.send(OnnxResponse::GnnResult { context_vector });
                    }
                }
            }
        });

        Self { tx }
    }

    pub fn get_tx(&self) -> Sender<OnnxRequest> {
        self.tx.clone()
    }

    pub fn send(&self, request: OnnxRequest) {
        let _ = self.tx.send(request);
    }
}
