//! Optional Miniscript policy support.
//!
//! This module is behind the `miniscript-policy` feature. It currently provides
//! policy compilation and safety checks for Taproot descriptors. Spending support
//! should be built separately because script-path signing also needs witness data,
//! control blocks, and satisfaction inputs.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use miniscript::descriptor::DescriptorType;
use miniscript::policy::Concrete;
use miniscript::Descriptor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyCompileResult {
    pub policy: String,
    pub descriptor: String,
    pub descriptor_type: String,
    pub segwit_version: String,
    pub is_taproot: bool,
    pub is_sane: bool,
    pub internal_key: String,
    pub warning: String,
}

pub fn compile_taproot_policy(
    policy: &str,
    internal_key: Option<&str>,
) -> Result<PolicyCompileResult> {
    let normalized_policy = normalize_policy(policy);
    let internal_key = internal_key
        .filter(|key| !key.trim().is_empty())
        .unwrap_or("UNSPENDABLE_KEY")
        .trim()
        .to_string();

    let concrete = Concrete::<String>::from_str(&normalized_policy)
        .with_context(|| format!("failed to parse miniscript policy '{}'", policy))?;
    let descriptor = concrete
        .compile_tr(Some(internal_key.clone()))
        .context("failed to compile policy as Taproot descriptor")?;

    descriptor
        .sanity_check()
        .context("compiled descriptor failed miniscript sanity check")?;

    let descriptor_type = descriptor.desc_type();
    let segwit_version = descriptor_type
        .segwit_version()
        .map(|version| format!("{version:?}"))
        .unwrap_or_else(|| "none".to_string());

    Ok(PolicyCompileResult {
        policy: normalized_policy,
        descriptor: descriptor.to_string(),
        descriptor_type: descriptor_type_name(descriptor_type).to_string(),
        segwit_version,
        is_taproot: matches!(descriptor, Descriptor::Tr(_)),
        is_sane: true,
        internal_key,
        warning: "Compile/preview only. Script-path spending still requires witness/control-block integration."
            .to_string(),
    })
}

fn normalize_policy(policy: &str) -> String {
    policy
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
}

fn descriptor_type_name(descriptor_type: DescriptorType) -> &'static str {
    match descriptor_type {
        DescriptorType::Bare => "bare",
        DescriptorType::Sh => "sh",
        DescriptorType::Wsh => "wsh",
        DescriptorType::Wpkh => "wpkh",
        DescriptorType::ShWsh => "sh-wsh",
        DescriptorType::ShWpkh => "sh-wpkh",
        DescriptorType::Pkh => "pkh",
        DescriptorType::ShSortedMulti => "sh-sortedmulti",
        DescriptorType::WshSortedMulti => "wsh-sortedmulti",
        DescriptorType::ShWshSortedMulti => "sh-wsh-sortedmulti",
        DescriptorType::Tr => "tr",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_threshold_taproot_policy() {
        let result =
            compile_taproot_policy("thresh(2,pk(A),pk(B),pk(C))", Some("INTERNAL")).unwrap();

        assert_eq!(result.descriptor_type, "tr");
        assert_eq!(result.segwit_version, "V1");
        assert!(result.is_taproot);
        assert!(result.is_sane);
        assert!(result.descriptor.starts_with("tr("));
        assert!(result.descriptor.contains("multi_a(2,A,B,C)"));
    }

    #[test]
    fn strips_policy_whitespace() {
        let result = compile_taproot_policy(
            "or(
                pk(A),
                and(pk(B),older(144))
            )",
            Some("INTERNAL"),
        )
        .unwrap();

        assert_eq!(result.policy, "or(pk(A),and(pk(B),older(144)))");
        assert!(result.descriptor.starts_with("tr("));
    }

    #[test]
    fn rejects_invalid_policy() {
        assert!(compile_taproot_policy("not-a-policy", Some("INTERNAL")).is_err());
    }
}
