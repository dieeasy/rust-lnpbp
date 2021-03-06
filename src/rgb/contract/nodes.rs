// LNP/BP Rust Library
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use std::collections::BTreeSet;

use amplify::AsAny;
use bitcoin::hashes::Hash;

use super::{
    Assignments, AutoConceal, OwnedRights, ParentOwnedRights,
    ParentPublicRights,
};
use crate::bp;
use crate::client_side_validation::{
    commit_strategy, CommitEncodeWithStrategy, ConsensusCommit,
};
use crate::paradigms::client_side_validation::CommitEncode;
use crate::rgb::schema::{
    ExtensionType, FieldType, NodeType, OwnedRightType, PublicRightType,
    TransitionType,
};
use crate::rgb::{schema, seal, Metadata, SchemaId, SimplicityScript};

/// Holds definition of valencies for contract nodes, which is a set of
/// allowed valencies types
pub type PublicRights = BTreeSet<PublicRightType>;

static MIDSTATE_NODE_ID: [u8; 32] = [
    0x90, 0xd0, 0xc4, 0x9b, 0xa6, 0xb8, 0xa, 0x5b, 0xbc, 0xba, 0x19, 0x9, 0xdc,
    0xbd, 0x5a, 0x58, 0x55, 0x6a, 0xe2, 0x16, 0xa5, 0xee, 0xb7, 0x3c, 0x1,
    0xe0, 0x86, 0x91, 0x22, 0x43, 0x12, 0x9f,
];

sha256t_hash_newtype!(
    NodeId,
    NodeIdTag,
    MIDSTATE_NODE_ID,
    64,
    doc = "Unique node (genesis, extensions & state transition) identifier \
           equivalent to the commitment hash",
    false
);

impl CommitEncodeWithStrategy for NodeId {
    type Strategy = commit_strategy::UsingStrict;
}

sha256t_hash_newtype!(
    ContractId,
    ContractIdTag,
    MIDSTATE_NODE_ID,
    64,
    doc = "Unique contract identifier equivalent to the contract genesis \
           commitment hash",
    true
);

impl From<ContractId> for crate::bp::chain::AssetId {
    fn from(id: ContractId) -> Self {
        Self::from_inner(id.into_inner())
    }
}

impl CommitEncodeWithStrategy for ContractId {
    type Strategy = commit_strategy::UsingStrict;
}

/// Trait which is implemented by all node types (see [`NodeType`])
pub trait Node: AsAny {
    /// Returns type of the node (see [`NodeType`]). Unfortunately, this can't
    /// be just a const, since it will break our ability to convert concrete
    /// `Node` types into `&dyn Node` (entities implementing traits with const
    /// definitions can't be made into objects)
    fn node_type(&self) -> NodeType;

    /// Returns [`NodeId`], which is a hash of this node commitment
    /// serialization
    fn node_id(&self) -> NodeId;

    /// Returns [`Option::Some`]`(`[`ContractId`]`)`, which is a hash of
    /// genesis.
    /// - For genesis node, this hash is byte-equal to [`NodeId`] (however
    ///   displayed in a reverse manner, to introduce semantical distinction)
    /// - For extension node function returns id of the genesis, to which this
    ///   node commits to
    /// - For state transition function returns [`Option::None`], since they do
    ///   not keep this information; it must be deduced through state transition
    ///   graph
    fn contract_id(&self) -> Option<ContractId>;

    /// Returns [`Option::Some`]`(`[`TransitionType`]`)` for transitions or
    /// [`Option::None`] for genesis and extension node types
    fn transition_type(&self) -> Option<TransitionType>;

    /// Returns [`Option::Some`]`(`[`ExtensionType`]`)` for extension nodes or
    /// [`Option::None`] for genesis and trate transitions
    fn extension_type(&self) -> Option<ExtensionType>;

    fn metadata(&self) -> &Metadata;
    fn parent_owned_rights(&self) -> &ParentOwnedRights;
    fn parent_public_rights(&self) -> &ParentPublicRights;
    fn owned_rights(&self) -> &OwnedRights;
    fn owned_rights_mut(&mut self) -> &mut OwnedRights;
    fn public_rights(&self) -> &PublicRights;
    fn public_rights_mut(&mut self) -> &mut PublicRights;
    fn script(&self) -> &SimplicityScript;

    #[inline]
    fn field_types(&self) -> Vec<FieldType> {
        self.metadata().keys().cloned().collect()
    }

    #[inline]
    fn owned_right_types(&self) -> BTreeSet<OwnedRightType> {
        self.owned_rights().keys().cloned().collect()
    }

    #[inline]
    fn owned_rights_by_type(&self, t: OwnedRightType) -> Option<&Assignments> {
        self.owned_rights().into_iter().find_map(|(t2, a)| {
            if *t2 == t {
                Some(a)
            } else {
                None
            }
        })
    }

    #[inline]
    fn all_seal_definitions(&self) -> Vec<seal::Confidential> {
        self.owned_rights()
            .into_iter()
            .flat_map(|(_, assignment)| assignment.all_seal_definitions())
            .collect()
    }

    #[inline]
    fn known_seal_definitions(&self) -> Vec<seal::Revealed> {
        self.owned_rights()
            .into_iter()
            .flat_map(|(_, assignment)| assignment.known_seal_definitions())
            .collect()
    }

    #[inline]
    fn known_seal_definitions_by_type(
        &self,
        assignment_type: OwnedRightType,
    ) -> Vec<seal::Revealed> {
        self.owned_rights_by_type(assignment_type)
            .map(Assignments::known_seal_definitions)
            .unwrap_or(vec![])
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default, AsAny)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct Genesis {
    schema_id: SchemaId,
    chain: bp::Chain,
    metadata: Metadata,
    owned_rights: OwnedRights,
    public_rights: PublicRights,
    script: SimplicityScript,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Default, StrictEncode, StrictDecode, AsAny,
)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
#[lnpbp_crate(crate)]
pub struct Extension {
    extension_type: ExtensionType,
    contract_id: ContractId,
    metadata: Metadata,
    parent_public_rights: ParentPublicRights,
    owned_rights: OwnedRights,
    public_rights: PublicRights,
    script: SimplicityScript,
}

#[derive(
    Clone, PartialEq, Eq, Debug, Default, StrictEncode, StrictDecode, AsAny,
)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
#[lnpbp_crate(crate)]
pub struct Transition {
    transition_type: TransitionType,
    metadata: Metadata,
    parent_owned_rights: ParentOwnedRights,
    owned_rights: OwnedRights,
    public_rights: PublicRights,
    script: SimplicityScript,
}

impl ConsensusCommit for Genesis {
    type Commitment = NodeId;
}

impl ConsensusCommit for Extension {
    type Commitment = NodeId;
}

impl ConsensusCommit for Transition {
    type Commitment = NodeId;
}

impl CommitEncodeWithStrategy for Extension {
    type Strategy = commit_strategy::UsingStrict;
}

impl CommitEncodeWithStrategy for Transition {
    type Strategy = commit_strategy::UsingStrict;
}

impl AutoConceal for Genesis {
    fn conceal_except(&mut self, seals: &Vec<seal::Confidential>) -> usize {
        let mut count = 0;
        for (_, assignment) in self.owned_rights_mut() {
            count += assignment.conceal_except(seals);
        }
        count
    }
}

impl AutoConceal for Extension {
    fn conceal_except(&mut self, seals: &Vec<seal::Confidential>) -> usize {
        let mut count = 0;
        for (_, assignment) in self.owned_rights_mut() {
            count += assignment.conceal_except(seals);
        }
        count
    }
}

impl AutoConceal for Transition {
    fn conceal_except(&mut self, seals: &Vec<seal::Confidential>) -> usize {
        let mut count = 0;
        for (_, assignment) in self.owned_rights_mut() {
            count += assignment.conceal_except(seals);
        }
        count
    }
}

impl Node for Genesis {
    #[inline]
    fn node_type(&self) -> NodeType {
        NodeType::Genesis
    }

    #[inline]
    fn node_id(&self) -> NodeId {
        self.clone().consensus_commit()
    }

    #[inline]
    fn contract_id(&self) -> Option<ContractId> {
        Some(ContractId::from_inner(self.node_id().into_inner()))
    }

    #[inline]
    fn transition_type(&self) -> Option<schema::TransitionType> {
        None
    }

    #[inline]
    fn extension_type(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn parent_owned_rights(&self) -> &ParentOwnedRights {
        lazy_static! {
            static ref PARENT_EMPTY: ParentOwnedRights =
                ParentOwnedRights::new();
        }
        &PARENT_EMPTY
    }

    #[inline]
    fn parent_public_rights(&self) -> &ParentPublicRights {
        lazy_static! {
            static ref PARENT_EMPTY: ParentPublicRights =
                ParentPublicRights::new();
        }
        &PARENT_EMPTY
    }

    #[inline]
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    #[inline]
    fn owned_rights(&self) -> &OwnedRights {
        &self.owned_rights
    }

    #[inline]
    fn owned_rights_mut(&mut self) -> &mut OwnedRights {
        &mut self.owned_rights
    }

    #[inline]
    fn public_rights(&self) -> &PublicRights {
        &self.public_rights
    }

    #[inline]
    fn public_rights_mut(&mut self) -> &mut PublicRights {
        &mut self.public_rights
    }

    #[inline]
    fn script(&self) -> &SimplicityScript {
        &self.script
    }
}

impl Node for Extension {
    #[inline]
    fn node_type(&self) -> NodeType {
        NodeType::Extension
    }

    #[inline]
    fn node_id(&self) -> NodeId {
        self.clone().consensus_commit()
    }

    #[inline]
    fn contract_id(&self) -> Option<ContractId> {
        Some(self.contract_id)
    }

    #[inline]
    fn transition_type(&self) -> Option<schema::TransitionType> {
        None
    }

    #[inline]
    fn extension_type(&self) -> Option<usize> {
        Some(self.extension_type)
    }

    #[inline]
    fn parent_owned_rights(&self) -> &ParentOwnedRights {
        lazy_static! {
            static ref PARENT_EMPTY: ParentOwnedRights =
                ParentOwnedRights::new();
        }
        &PARENT_EMPTY
    }

    #[inline]
    fn parent_public_rights(&self) -> &ParentPublicRights {
        &self.parent_public_rights
    }

    #[inline]
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    #[inline]
    fn owned_rights(&self) -> &OwnedRights {
        &self.owned_rights
    }

    #[inline]
    fn owned_rights_mut(&mut self) -> &mut OwnedRights {
        &mut self.owned_rights
    }

    #[inline]
    fn public_rights(&self) -> &PublicRights {
        &self.public_rights
    }

    #[inline]
    fn public_rights_mut(&mut self) -> &mut PublicRights {
        &mut self.public_rights
    }

    #[inline]
    fn script(&self) -> &SimplicityScript {
        &self.script
    }
}

impl Node for Transition {
    #[inline]
    fn node_type(&self) -> NodeType {
        NodeType::StateTransition
    }

    #[inline]
    fn node_id(&self) -> NodeId {
        self.clone().consensus_commit()
    }

    #[inline]
    fn contract_id(&self) -> Option<ContractId> {
        None
    }

    #[inline]
    fn transition_type(&self) -> Option<schema::TransitionType> {
        Some(self.transition_type)
    }

    #[inline]
    fn extension_type(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn parent_owned_rights(&self) -> &ParentOwnedRights {
        &self.parent_owned_rights
    }

    #[inline]
    fn parent_public_rights(&self) -> &ParentPublicRights {
        lazy_static! {
            static ref PARENT_EMPTY: ParentPublicRights =
                ParentPublicRights::new();
        }
        &PARENT_EMPTY
    }

    #[inline]
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    #[inline]
    fn owned_rights(&self) -> &OwnedRights {
        &self.owned_rights
    }

    #[inline]
    fn owned_rights_mut(&mut self) -> &mut OwnedRights {
        &mut self.owned_rights
    }

    #[inline]
    fn public_rights(&self) -> &PublicRights {
        &self.public_rights
    }

    #[inline]
    fn public_rights_mut(&mut self) -> &mut PublicRights {
        &mut self.public_rights
    }

    #[inline]
    fn script(&self) -> &SimplicityScript {
        &self.script
    }
}

impl Genesis {
    pub fn with(
        schema_id: SchemaId,
        chain: bp::Chain,
        metadata: Metadata,
        owned_rights: OwnedRights,
        public_rights: PublicRights,
        script: SimplicityScript,
    ) -> Self {
        Self {
            schema_id,
            chain,
            metadata,
            owned_rights,
            public_rights,
            script,
        }
    }

    #[inline]
    pub fn contract_id(&self) -> ContractId {
        ContractId::from_inner(self.node_id().into_inner())
    }

    #[inline]
    pub fn schema_id(&self) -> SchemaId {
        self.schema_id
    }

    #[inline]
    pub fn chain(&self) -> &bp::Chain {
        &self.chain
    }
}

impl Extension {
    pub fn with(
        extension_type: ExtensionType,
        contract_id: ContractId,
        metadata: Metadata,
        parent_public_rights: ParentPublicRights,
        owned_rights: OwnedRights,
        public_rights: PublicRights,
        script: SimplicityScript,
    ) -> Self {
        Self {
            extension_type,
            contract_id,
            metadata,
            parent_public_rights,
            owned_rights,
            public_rights,
            script,
        }
    }
}

impl Transition {
    pub fn with(
        transition_type: schema::TransitionType,
        metadata: Metadata,
        parent_owned_rights: ParentOwnedRights,
        owned_rights: OwnedRights,
        public_rights: PublicRights,
        script: SimplicityScript,
    ) -> Self {
        Self {
            transition_type,
            metadata,
            parent_owned_rights,
            owned_rights,
            public_rights,
            script,
        }
    }
}

mod strict_encoding {
    use super::*;
    use crate::strict_encoding::{
        strategies, strict_decode, strict_encode, Error, Strategy,
        StrictDecode, StrictEncode,
    };
    use std::io;

    impl Strategy for NodeId {
        type Strategy = strategies::HashFixedBytes;
    }

    impl Strategy for ContractId {
        type Strategy = strategies::HashFixedBytes;
    }

    // ![CONSENSUS-CRITICAL]: Commit encode is different for genesis from strict
    //                        encode since we only commit to chain genesis block
    //                        hash and not all chain parameters.
    // See <https://github.com/LNP-BP/LNPBPs/issues/58> for details.
    impl CommitEncode for Genesis {
        fn commit_encode<E: io::Write>(self, mut e: E) -> usize {
            let mut encoder = || -> Result<_, Error> {
                let mut len = self.schema_id.strict_encode(&mut e)?;
                len += self.chain.as_genesis_hash().strict_encode(&mut e)?;
                Ok(strict_encode_list!(e; len;
                    self.metadata,
                    self.owned_rights,
                    self.public_rights,
                    self.script
                ))
            };
            encoder().expect("Strict encoding of genesis data must not fail")
        }
    }

    impl StrictEncode for Genesis {
        type Error = Error;

        fn strict_encode<E: io::Write>(
            &self,
            mut e: E,
        ) -> Result<usize, Error> {
            let chain_params = strict_encode(&self.chain)?;
            Ok(strict_encode_list!(e;
                self.schema_id,
                // ![NETWORK-CRITICAL]: Chain params fields may update, so we
                //                      will serialize chain parameters in all
                //                      known/legacy formats for compatibility.
                //                      Thus, they are serialized as a vector
                //                      of byte strings, each one representing
                //                      a next version of chain parameters
                //                      representation.
                // <https://github.com/LNP-BP/rust-lnpbp/issues/114>
                1usize,
                chain_params,
                self.metadata,
                self.owned_rights,
                self.public_rights,
                self.script
            ))
        }
    }

    impl StrictDecode for Genesis {
        type Error = Error;

        fn strict_decode<D: io::Read>(mut d: D) -> Result<Self, Error> {
            let schema_id = SchemaId::strict_decode(&mut d)?;
            let chain_params_no = usize::strict_decode(&mut d)?;
            if chain_params_no < 1 {
                Err(Error::ValueOutOfRange(
                    "genesis must contain at least one `chain_param` data structure",
                    1u128..(core::u16::MAX as u128),
                    0,
                ))?
            }
            let chain_data = Vec::<u8>::strict_decode(&mut d)?;
            let chain = strict_decode(&chain_data)?;
            for _ in 1..chain_params_no {
                // Ignoring the rest of chain parameters
                let _ = Vec::<u8>::strict_decode(&mut d)?;
            }
            let metadata = Metadata::strict_decode(&mut d)?;
            let assignments = OwnedRights::strict_decode(&mut d)?;
            let valencies = PublicRights::strict_decode(&mut d)?;
            let script = SimplicityScript::strict_decode(&mut d)?;
            Ok(Self {
                schema_id,
                chain,
                metadata,
                owned_rights: assignments,
                public_rights: valencies,
                script,
            })
        }
    }
}

#[cfg(test)]
mod test {
    use amplify::Wrapper;
    use bitcoin_hashes::hex::{FromHex, ToHex};
    use std::io::Write;

    use super::*;
    use crate::bp::chain::{Chain, GENESIS_HASH_MAINNET};
    use crate::bp::tagged_hash;
    use crate::commit_verify::CommitVerify;
    use crate::paradigms::strict_encoding::{test::*, StrictDecode};
    use crate::strict_encoding::{strict_encode, StrictEncode};

    static TRANSITION: [u8; 2364] = include!("../../../test/transition.in");
    static GENESIS: [u8; 2462] = include!("../../../test/genesis.in");

    #[test]
    fn test_node_id_midstate() {
        let midstate = tagged_hash::Midstate::with(b"rgb:node");
        assert_eq!(midstate.into_inner(), MIDSTATE_NODE_ID);
    }

    // Making sure that <https://github.com/LNP-BP/LNPBPs/issues/58>
    // is fulfilled and we do not occasionally commit to all chain
    // parameters (which may vary and change with time) in RGB contract id
    #[test]
    fn test_genesis_commit_ne_strict() {
        let genesis = Genesis {
            schema_id: Default::default(),
            chain: Chain::Mainnet,
            metadata: Default::default(),
            owned_rights: Default::default(),
            public_rights: Default::default(),
            script: Default::default(),
        };
        assert_ne!(
            strict_encode(&genesis).unwrap(),
            genesis.clone().consensus_commit().to_vec()
        );

        let mut encoder = vec![];
        genesis.schema_id.strict_encode(&mut encoder).unwrap();
        encoder.write_all(GENESIS_HASH_MAINNET).unwrap();
        genesis.metadata.strict_encode(&mut encoder).unwrap();
        genesis.owned_rights.strict_encode(&mut encoder).unwrap();
        genesis.public_rights.strict_encode(&mut encoder).unwrap();
        genesis.script.strict_encode(&mut encoder).unwrap();
        assert_eq!(genesis.consensus_commit(), NodeId::commit(&encoder));

        let transition = Transition {
            transition_type: Default::default(),
            metadata: Default::default(),
            parent_owned_rights: Default::default(),
            owned_rights: Default::default(),
            public_rights: Default::default(),
            script: Default::default(),
        };

        let mut encoder = vec![];
        transition
            .transition_type
            .strict_encode(&mut encoder)
            .unwrap();
        transition.metadata.strict_encode(&mut encoder).unwrap();
        transition
            .parent_owned_rights
            .strict_encode(&mut encoder)
            .unwrap();
        transition.owned_rights.strict_encode(&mut encoder).unwrap();
        transition
            .public_rights
            .strict_encode(&mut encoder)
            .unwrap();
        transition.script.strict_encode(&mut encoder).unwrap();

        let mut encoder1 = vec![];
        let mut encoder2 = vec![];

        transition.clone().commit_encode(&mut encoder1);
        transition.clone().strict_encode(&mut encoder2).unwrap();

        assert_eq!(encoder1, encoder2);
        assert_eq!(encoder, encoder1);
    }

    #[test]
    fn test_encoding_nodes() {
        test_encode!((GENESIS, Genesis));
        test_encode!((TRANSITION, Transition));
    }

    #[test]
    fn test_node_attributes() {
        let genesis = Genesis::strict_decode(&GENESIS[..]).unwrap();
        let transition = Transition::strict_decode(&TRANSITION[..]).unwrap();

        // Typeid/Nodeid test
        assert_eq!(
            genesis.node_id().to_hex(),
            "150fac8310ed05413019094911dc1025801e1303bdd96c2bb926aede2a8420d0"
        );
        assert_eq!(
            transition.node_id().to_hex(),
            "d44fabfcc3f96af2f7d3d3c5871df281d84768fc5bd0c00602f2b96cdd8e6294"
        );

        assert_eq!(genesis.transition_type(), None);
        assert_eq!(transition.transition_type(), Some(10));

        // Ancestor test

        assert_eq!(genesis.parent_owned_rights(), &ParentOwnedRights::new());

        let ancestor_trn = transition.parent_owned_rights();
        let assignments = ancestor_trn
            .get(
                &NodeId::from_hex(
                    "f57ed27ee4199072c5ff3b774febc94d26d3e4a5559d133de4750a948df50e06",
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            assignments.get(&1usize).unwrap(),
            &[1u16, 2u16, 3u16, 4u16, 5u16].to_vec()
        );
        assert_eq!(
            assignments.get(&2usize).unwrap(),
            &[10u16, 20u16, 30u16, 40u16, 50u16].to_vec()
        );
        assert_eq!(
            assignments.get(&3usize).unwrap(),
            &[100u16, 200u16, 300u16, 400u16, 500u16].to_vec()
        );

        // Metadata test

        let gen_meta = genesis.metadata();

        let tran_meta = transition.metadata();

        assert_eq!(gen_meta, tran_meta);

        let u8_from_gen = gen_meta.u8(13 as schema::FieldType);

        assert_eq!(u8_from_gen, [2u8, 3u8].to_vec());

        let string_from_tran = tran_meta.string(13 as schema::FieldType);

        assert_eq!(string_from_tran[0], "One Random String".to_string());

        // Assignments test

        let gen_assignments = genesis.owned_rights();
        let tran_assingmnets = transition.owned_rights();

        assert_eq!(gen_assignments, tran_assingmnets);

        assert!(gen_assignments.get(&1usize).unwrap().is_declarative_state());
        assert!(gen_assignments.get(&2usize).unwrap().is_discrete_state());
        assert!(tran_assingmnets.get(&3usize).unwrap().is_custom_state());

        let seal1 = gen_assignments
            .get(&2usize)
            .unwrap()
            .seal_definition(1)
            .unwrap()
            .unwrap();

        let txid = match seal1 {
            super::seal::Revealed::TxOutpoint(op) => Some(op.txid),
            _ => None,
        }
        .unwrap();

        assert_eq!(
            txid.to_hex(),
            "201fdd1e2b62d7b6938271295118ee181f1bac5e57d9f4528925650d36d3af8e"
                .to_string()
        );

        let seal2 = tran_assingmnets
            .get(&3usize)
            .unwrap()
            .seal_definition(1)
            .unwrap()
            .unwrap();

        let txid = match seal2 {
            super::seal::Revealed::TxOutpoint(op) => Some(op.txid),
            _ => None,
        }
        .unwrap();

        assert_eq!(
            txid.to_hex(),
            "f57ed27ee4199072c5ff3b774febc94d26d3e4a5559d133de4750a948df50e06"
                .to_string()
        );

        // Script
        let gen_script = genesis.script();
        let tran_script = transition.script();

        assert_eq!(gen_script, tran_script);

        assert_eq!(gen_script, &[1, 2, 3, 4, 5]);

        // Field Types
        let gen_fields = genesis.field_types();
        let tran_fields = transition.field_types();

        assert_eq!(gen_fields, tran_fields);

        assert_eq!(gen_fields, vec![13usize]);

        // Assignment types
        let gen_ass_types = genesis.owned_right_types();
        let tran_ass_types = transition.owned_right_types();

        assert_eq!(gen_ass_types, tran_ass_types);

        assert_eq!(gen_ass_types, bset![1usize, 2, 3]);

        // assignment by types
        let assignment_gen = genesis.owned_rights_by_type(3).unwrap();
        let assignment_tran = transition.owned_rights_by_type(1).unwrap();

        assert!(assignment_gen.is_custom_state());
        assert!(assignment_tran.is_declarative_state());

        // All seal confidentials
        let gen_seals = genesis.all_seal_definitions();
        let tran_seals = transition.all_seal_definitions();

        assert_eq!(gen_seals, tran_seals);

        assert_eq!(
            gen_seals[0].to_hex(),
            "6b3c1bee0bd431f53e6c099890fdaf51b8556a6dcd61c6150ca055d0e1d4a524"
                .to_string()
        );
        assert_eq!(
            tran_seals[3].to_hex(),
            "58f3ea4817a12aa6f1007d5b3d24dd2940ce40f8498029e05f1dc6465b3d65b4"
                .to_string()
        );

        // Known seals
        let known_gen_seals = genesis.known_seal_definitions();
        let known_seals_tran = transition.known_seal_definitions();

        assert_eq!(known_gen_seals, known_seals_tran);

        let txid1 = match known_gen_seals[2] {
            super::seal::Revealed::TxOutpoint(op) => Some(op.txid),
            _ => None,
        }
        .unwrap();

        let txid2 = match known_gen_seals[3] {
            super::seal::Revealed::TxOutpoint(op) => Some(op.txid),
            _ => None,
        }
        .unwrap();

        assert_eq!(
            txid1.to_hex(),
            "f57ed27ee4199072c5ff3b774febc94d26d3e4a5559d133de4750a948df50e06"
                .to_string()
        );
        assert_eq!(
            txid2.to_hex(),
            "201fdd1e2b62d7b6938271295118ee181f1bac5e57d9f4528925650d36d3af8e"
                .to_string()
        );

        // Known seals by type
        let dec_gen_seals = genesis.known_seal_definitions_by_type(1);
        let hash_tran_seals = transition.known_seal_definitions_by_type(3);

        let txid1 = match dec_gen_seals[0] {
            super::seal::Revealed::TxOutpoint(op) => Some(op.txid),
            _ => None,
        }
        .unwrap();

        assert_eq!(
            txid1.to_hex(),
            "f57ed27ee4199072c5ff3b774febc94d26d3e4a5559d133de4750a948df50e06"
                .to_string()
        );

        let txid2 = match hash_tran_seals[1] {
            super::seal::Revealed::TxOutpoint(op) => Some(op.txid),
            _ => None,
        }
        .unwrap();

        assert_eq!(
            txid2.to_hex(),
            "201fdd1e2b62d7b6938271295118ee181f1bac5e57d9f4528925650d36d3af8e"
                .to_string()
        );
    }

    #[test]
    fn test_autoconceal_node() {
        let mut genesis = Genesis::strict_decode(&GENESIS[..]).unwrap();
        let mut transition =
            Transition::strict_decode(&TRANSITION[..]).unwrap();

        assert_eq!(
            genesis.clone().consensus_commit(),
            NodeId::from_hex("150fac8310ed05413019094911dc1025801e1303bdd96c2bb926aede2a8420d0")
                .unwrap()
        );
        assert_eq!(
            transition.clone().consensus_commit(),
            NodeId::from_hex("d44fabfcc3f96af2f7d3d3c5871df281d84768fc5bd0c00602f2b96cdd8e6294")
                .unwrap()
        );

        genesis.conceal_all();
        transition.conceal_all();

        assert_eq!(
            genesis.clone().consensus_commit(),
            NodeId::from_hex("ec9ae8acf89a944049f7ba084145ae97a2a44f6f20a61aefe438983028598e97")
                .unwrap()
        );
        assert_eq!(
            transition.clone().consensus_commit(),
            NodeId::from_hex("154258e56df3422e6897278650d0acace8c04ea8a93d0c9cb6f081053b13534e")
                .unwrap()
        );
    }

    #[test]
    fn test_genesis_impl() {
        let genesis: Genesis = Genesis::strict_decode(&GENESIS[..]).unwrap();

        let contractid = genesis.contract_id();
        let schemaid = genesis.schema_id();
        let chain = genesis.chain();

        assert_eq!(
            contractid,
            ContractId::from_hex(
                "d020842adeae26b92b6cd9bd03131e802510dc11490919304105ed1083ac0f15"
            )
            .unwrap()
        );
        assert_eq!(
            schemaid,
            SchemaId::from_hex("201fdd1e2b62d7b6938271295118ee181f1bac5e57d9f4528925650d36d3af8e")
                .unwrap()
        );
        assert_eq!(chain, &bp::chain::Chain::Mainnet);
    }
}
