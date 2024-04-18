use libsecp256k1::{RecoveryId, Signature};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};

/// A Dria Computation payload is a triple:
///
/// 1. Ciphertext: Computation result encrypted with the public key of the task.
/// 2. Commitment: A commitment to `signature || plaintext result`
/// 3. Signature: A signature on the digest of plaintext result.
///
/// To check the commitment, one must decrypt the ciphertext and parse plaintext from it,
/// and compute the digest using SHA256. That digest will then be used for the signature check.
#[derive(Serialize, Deserialize, Debug)]
pub struct Payload {
    pub signature: String,
    pub ciphertext: String,
    pub commitment: String,
}

impl From<Payload> for String {
    fn from(value: Payload) -> Self {
        to_string(&json!(Payload::from(value))).unwrap()
    }
}

// /// A signature is 65-bytes made of `r || s || v`.
// /// This struct stores each field explicitly.
// // #[derive(Serialize, Deserialize, Debug)]
// // pub struct PayloadSignature {
// //     v: u8,
// //     r: [u8; 32],
// //     s: [u8; 32],
// // }

// impl From<PayloadSignature> for String {
//     fn from(value: PayloadSignature) -> Self {
//         format!(
//             "{}{}{}",
//             hex::encode(value.r),
//             hex::encode(value.s),
//             hex::encode(vec![value.v])
//         )
//     }
// }

// impl From<(Signature, RecoveryId)> for PayloadSignature {
//     fn from(value: (Signature, RecoveryId)) -> Self {
//         Self {
//             v: value.1.serialize(),
//             r: value.0.r.b32(),
//             s: value.0.s.b32(),
//         }
//     }
// }

// impl From<PayloadSignature> for (Signature, RecoveryId) {
//     fn from(value: PayloadSignature) -> Self {
//         let mut slice = vec![];
//         slice.extend_from_slice(value.r.as_ref());
//         slice.extend_from_slice(value.r.as_ref());
//         let signature =
//             Signature::parse_standard_slice(slice.as_ref()).expect("Could not parse Signature.");

//         let recid = RecoveryId::parse(value.v).expect("Could not parse RecoveryId.");
//         (signature, recid)
//     }
// }
