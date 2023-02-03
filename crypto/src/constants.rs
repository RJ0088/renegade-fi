//! Defines important constants used in the
#![allow(non_snake_case)]

use memoize::memoize;
use num_bigint::BigUint;

use crate::fields::DalekRistrettoField;

/// The maximum number of orders allowed in a wallet
pub const MAX_ORDERS: usize = 2;
/// The maximum number of balances allowed in a wallet
pub const MAX_BALANCES: usize = 2;

///
/// Below are:
///     1. The MDS matrix (https://en.wikipedia.org/wiki/MDS_matrix) used in between SBoxes
///     2. The round constants added to the input of each round
/// These were generated using the scripts published by the Poseidon authors located here:
///     https://extgit.iaik.tugraz.at/krypto/hadeshash
/// The MDS matrix was generated by running:
///     sage generate_params_poseidon.sage 1 0 254 3 8 56 <field_prime>
/// Where the field prime is the hex string of the prime defined here:
///   https://docs.rs/curve25519-dalek/0.18.0/curve25519_dalek/scalar/struct.Scalar.html
///   i.e. 1000000000000000000000000000000014DEF9DEA2F79CD65812631A5CF5D3ED
/// The round numbers (i.e. R_f = 8 and R_p = 56) were generated by
///     python3 calc_round_numbers.py
/// from the scripts above and taking the output for t = 3, \alpha = 5
#[memoize]
pub fn POSEIDON_MDS_MATRIX_T_3() -> Vec<Vec<DalekRistrettoField>> {
    vec![
        vec![
            field_element_from_hex_string(
                b"28b15d6eed95eea7ebb45451308179edb7202e21a753d9cb368316b3d285219",
            ),
            field_element_from_hex_string(
                b"74644036d7bfabfb4e77949ca57cb10cc53cb683f406ee11d0b199074334be8",
            ),
            field_element_from_hex_string(
                b"9b5c144f8266c0d667a4b1bb18bd1c4ad6ca9ebbafe27d804e4964234051282",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"e14502eb1fdcc85376cb9d7eaa622f17e692dc175ae0508442d8598f380265b",
            ),
            field_element_from_hex_string(
                b"521bec0db4e14a6fffad3ca794eab19618b0aec5bac29a8305df800b5fbc430",
            ),
            field_element_from_hex_string(
                b"db87bc574f48be56ea4eecd0f3d79d30ad08ad5d8a9a713575ada3251ad75d1",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"e2845dc7c8160c536291b53080bf3f9e1a0839cbf071a3a1fc5d3cbbf073bbc",
            ),
            field_element_from_hex_string(
                b"0b0a7dedbd7d5b8e2678f9f1978505a90d47020173d004ef48b305ac3674660",
            ),
            field_element_from_hex_string(
                b"b60fdf89da602fde4467b449aa34733d5e9e545230d8b16246676c0a7af06e7",
            ),
        ],
    ]
}

/// Round constants for t = 3 (2-1 hash)
#[memoize]
pub fn POSEIDON_ROUND_CONSTANTS_T_3() -> Vec<Vec<DalekRistrettoField>> {
    vec![
        vec![
            field_element_from_hex_string(
                b"27eed94d7f6f47c254d29fce05d73cf4358b38d6c01240710680538628bb758",
            ),
            field_element_from_hex_string(
                b"c31d0e4386f0ec82b3a1bb84024dc77602572ce7e40a7a360857ee750e52341",
            ),
            field_element_from_hex_string(
                b"28d3a4161b9a54e9134181f535a2acec7918ee8691eb681e50a6b2b6194af06",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"ac8b7a4a7deeb99784ce4ca353fb3e923e258bec248056bd72a02ccb3d3a4f1",
            ),
            field_element_from_hex_string(
                b"b9e55eaa236406af69d2b26d18379351d51421d7e7ae38b22abc7f74e94efa7",
            ),
            field_element_from_hex_string(
                b"2a2fffcc5e547e1638b14a57727ec32f31a1e56d17676be648591d19305dda1",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"68dc80f7274a0582feb0be68e7edeb121e3c0403356a150555fd6981a6ad531",
            ),
            field_element_from_hex_string(
                b"4471dfda42bea8d06d8a3d7c61c6bf96336935ba09fac616bcbf1db090277bc",
            ),
            field_element_from_hex_string(
                b"afc8d183acd550f0a15804056b6f0fd92be25ab2fc9ae40dd2dadfbba93392e",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"921cd06b16d052f6a1881db6f3cca8eb10f7614e8cecdf9bcbc5c4327b6be96",
            ),
            field_element_from_hex_string(
                b"68dcee3dc200e286fccbf1adedc0f046c883782b6d35f63666df3c40dff49b1",
            ),
            field_element_from_hex_string(
                b"6dcd41be11ef10d10c22cdc2e0ec848b76b75db2c2d2dcc49032decde25200c",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"7cfdd23464808c4a58a9357925e64bb942bd38e4e8120cf38a17d764fbd0fd6",
            ),
            field_element_from_hex_string(
                b"b711f837ce29dc086db1b6a51842e991d26c233e65136864e5522a8359a5d4d",
            ),
            field_element_from_hex_string(
                b"83a60bf57da1f86f54c5c6eb4c65658bb99e618bebcbd36e4de03ce8ff85328",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"97fc770582a4c47e2da7168340bf7cdc77dfa546bf8ef32b4a852e735926ca4",
            ),
            field_element_from_hex_string(
                b"70e8d3c9011d9bda03979f25feaae3f119eba608d3d5260b5aa3fdb20a265e7",
            ),
            field_element_from_hex_string(
                b"d47ca5a459f89c6120c4b57c6be0a9179174a0099e78856055edca349e6da6d",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"a87e3d94d254d9b95ca1cd6ad1769dd6fb8947874a0487218dea03e3e8b1e33",
            ),
            field_element_from_hex_string(
                b"7acab9fde303b83806f96d431328b1936f13bb279aa1f1e0da934b927737576",
            ),
            field_element_from_hex_string(
                b"0169392b57b0b39a0e76043b0b27967e287935b1fefdfe24d051204432a028b",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"e33ac4e453f6dc62b00b63b2d9d3ae1eb68b221ebf929bf7b417ad2545dc5ec",
            ),
            field_element_from_hex_string(
                b"13371e93654416fa55819f40b824422f06747dab4549e6ebc617d35014ddda0",
            ),
            field_element_from_hex_string(
                b"617f158242af445821fe36e069268eab0feb7ab7a6a9c64cb90d661756d2ff5",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"d895930c370e74337d3fd5661a59c5dd7fc1cadddf5925554d603b0c660b426",
            ),
            field_element_from_hex_string(
                b"159bf308a6aa5cba675265162f184cd2363e23d0061cb2d0a43b35a4abbdb58",
            ),
            field_element_from_hex_string(
                b"62f01e3af097215ae033484a5c1745f28286f375aa1d129dcfe71762fe85ef4",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"7064925702f2a468cb67745599992ed477b123cde5b384093aff1b93e513c58",
            ),
            field_element_from_hex_string(
                b"09a9a9f2afbd1740fa9ff4c504eaaa3170760fc5c1d855b05e7c200ab52d623",
            ),
            field_element_from_hex_string(
                b"f3acb67aaccaf7fe695e1a9047a195b97e2422802775317232ff0891ca05b0f",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"5321cc718aa18a730080dbb74c900f0fef0c6c2dfa227541fc9f0c2ef059b49",
            ),
            field_element_from_hex_string(
                b"85fea25fd5e33419fa9739fe358c1cc7699bae940815ffaf33c9fa1795ef3ba",
            ),
            field_element_from_hex_string(
                b"726b127e6cc537413329c2638e282971dce56ee1eba89044cc810b2e9c75295",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"04da413ab0e5156adb5913823a39013422dcb2c0759c424d8b23b8751105d27",
            ),
            field_element_from_hex_string(
                b"8a6aba2706f9d9071afda8864e0c000a06e6e7b70f686033b9aeaa2ba3aadf7",
            ),
            field_element_from_hex_string(
                b"7b348b0d3afe5c6d9434c2827663a3d634648efeee7bf2e63b2907b45ea1c4a",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"acbd231b802a6b00379c947b511ab80d9fdad91772e6a9dc3f284bd317fa93b",
            ),
            field_element_from_hex_string(
                b"09e327c305512399520fee363a3154478651bac29ccc5635b2f5a9a150a89a5",
            ),
            field_element_from_hex_string(
                b"723ee11556f4736be1cc7665acbec25434398b3f0cf01e5235f838ddbd2a184",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"3b4f73a3f3a068ec01e0fafac01179a1dfa7f02328b9ffae3443c0331d68013",
            ),
            field_element_from_hex_string(
                b"6d60123303ae2b6b353df1d1b9b625e8241d2067895befb23fdd14628945324",
            ),
            field_element_from_hex_string(
                b"8123b1a8b117ff794e0561fbe9bfb8328f9569c0643e3e6c34d18527e389bc3",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"4f8b9302e1d1a74365031a50213ed0a61deabbb720c6ae3639b3b10df2a8e9b",
            ),
            field_element_from_hex_string(
                b"d382ec5d076d4705daf0877512235fe009c138f0e7e0525640f9eab56d61cc3",
            ),
            field_element_from_hex_string(
                b"2de08fd886a67f78cced64ae53c473eb21f81ab8deedc94028e5c14c7f527a8",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"610c1b7e39442f4fee9447fc98cb2ba534c5f311a0d627ed0aaf1abf412652f",
            ),
            field_element_from_hex_string(
                b"c2bf8622ffb95107ddc61b0291498cebc57b5a2d575a19f7daf95362d2bebca",
            ),
            field_element_from_hex_string(
                b"98f7d48b5c74c2828b997cbea6d93b3f09fc0174ea5a029a9e26d52bce31046",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"9203cbd4b29eabd5ff987cfa2633850c51c68100292e882289ab78501ad1187",
            ),
            field_element_from_hex_string(
                b"d58ee0f6f85720dfe82afbf6d96b680eb0e6ab73a46b09d27daf0edec4c4706",
            ),
            field_element_from_hex_string(
                b"befc33b076a4ca670e28d8ec6ecebd8fadebed4fc2322e5b29a88092860e71f",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"5298284f5cd468a359a4ebc9d23f3ea4cb82956e61e2ec25ad769cd882eb60a",
            ),
            field_element_from_hex_string(
                b"d4217d23420cacf2bc5138bb0d92aa796fdf7b84f07924d37cfffcccc227796",
            ),
            field_element_from_hex_string(
                b"475c51c5f14d2360083b6e99fb636ca3983b7267e996758e3e86c2e4f53c8d3",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"62aa51047838ea30e71b2d00a2604b1b5958bc95010d4e7a656ff8591f35574",
            ),
            field_element_from_hex_string(
                b"25c74aed69491c6062f39f524e261ea6d613bccd9ed4c926657bbc2517b0121",
            ),
            field_element_from_hex_string(
                b"10e2d7d4287c0bdb77dd75334ad1fd773849d01c94fb56d466409278253ce18",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"693ada333542141ddeaf050e0345a6ff2bda61dc8b522065c624b848a13d478",
            ),
            field_element_from_hex_string(
                b"5a285a6b50af09655ec7f68fd00d8a48dac1e25a83865144af05f5cd14877f1",
            ),
            field_element_from_hex_string(
                b"efeff432af8c1825280a6db9cdf39e31066a3f4ca411b8b2cc95c8582944d3e",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"dc59738a9f6c65747b2626fa13e5f2e29a6fc1b26a21520c0d433550ed9fbaf",
            ),
            field_element_from_hex_string(
                b"41acea4db63a90c39eb9bcd5e2d217b5d544f4e522d59dfb70b264b6962ce0d",
            ),
            field_element_from_hex_string(
                b"f80cb21ffbac8a796383bda480dfff22f27499188a645b1616038ce2dfc644a",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"b9cf4dd9aab8264168e9d019692fea62b0e5b09a115876790938b5ce646f3e5",
            ),
            field_element_from_hex_string(
                b"21f55689f190d7c8cffc6d5cc7b3fa7b3d14c96d8d65eb1fce6224e283234d9",
            ),
            field_element_from_hex_string(
                b"74bcd2763f5e667b96358f4f586ea6aff9a5834b72a4c41d5687cf3fa4cd5cb",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"fd6ee6f78d36b25d01c7d6a63eaf4ffbf40030cd83b895ebc22a5a24791e9b8",
            ),
            field_element_from_hex_string(
                b"b44d0116f885b569795c7bb2a8e10257b8c21749f8c8fe5595da97d50458669",
            ),
            field_element_from_hex_string(
                b"fed4eb1e45306f79b5607e406cdac1c6e2291722b8ac8afb9caaf01865b7a38",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"abe4626eea5a2b6cf8c9a7f625aa09a4565eea6a97f2feb6b8aa10da1e4e181",
            ),
            field_element_from_hex_string(
                b"9635d0ce00dc220a9a13d621772bc50b629fd39a7305d508ff5aa0d20032e01",
            ),
            field_element_from_hex_string(
                b"7eafde9d2a7b8519b07c5c12d86dcf26a6d02cd72ca594e3882f5d8a54a372e",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"cc99ef2522fabaf6247dd0dfece00c7138615ed7c91506dc5610c0c6a836a80",
            ),
            field_element_from_hex_string(
                b"e0823cfeeb4da16e2ee3c1b94222e9864f24047ec225116871e035eade45179",
            ),
            field_element_from_hex_string(
                b"397920b87151f76b4d2748724234680c4af0c0b227a1b902db03eba95ae4106",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"0a003b0aae34712f6e340a32fdb28cc23eb3ba146754c50ea6d5164f7b86304",
            ),
            field_element_from_hex_string(
                b"a97ee3cb227601eb632f3d04ec448c72ea7c2310279a6bf425d875c90d0b42f",
            ),
            field_element_from_hex_string(
                b"e174b8ed1f204c21125e317de5eb538445afd188d03262efde76b67426ecae0",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"f441e769ca519955c1b362136a93aeed5c85de5493876c70df47387c5bcde04",
            ),
            field_element_from_hex_string(
                b"912e2a81c7d3118bd10d1694d7ec91d88e4685c5af0dbdc6ac61a1b99694ddf",
            ),
            field_element_from_hex_string(
                b"d3fd92bc50ab51a60b29f1f469aff9eb882684a0a4b23d35c45e81d129e32b3",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"c639bc1390fe428113576ec8577738b32d224afd59be75278dbb760a96e7b19",
            ),
            field_element_from_hex_string(
                b"4c6af438f2035f595b7519c6878b4f647f664a783b28dc1d8dd0fdbc094f33e",
            ),
            field_element_from_hex_string(
                b"393741e2ea4576b77f71f4a2753ccca152e69f417e301dafbdf3c2d921c9539",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"8c032f434f395256e67380d3b69d753fb21111e8f42dbbd5a126c5f025b5a2e",
            ),
            field_element_from_hex_string(
                b"953e019bb6e53bae2a93adb719d774f9ee39191986866a7f45b77eb921124b8",
            ),
            field_element_from_hex_string(
                b"82038bfc5d06ab218dc599a212900f039b100c8978fa1bb8dc73e8a2db923aa",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"6d691cc27352425a6d7d4f8145fc9e3b4de600d080922976d00352747e7549d",
            ),
            field_element_from_hex_string(
                b"f0c5da0f78bebe6a6814e6b5aaa738e7dd1ce0364d58e183a7ce9d13edbfa6b",
            ),
            field_element_from_hex_string(
                b"28af384bd0db46b66b43b83008a2324d3c4a4f00b3161ee70f8a57b84ea8f41",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"be76d709063a38d44df89e51dce1aaccab5e1653a8fef54c86f0dad62d50e7f",
            ),
            field_element_from_hex_string(
                b"ba38c87108abb9460aba97ba1750fa2dfa557c5641725840bb84c7597d282c7",
            ),
            field_element_from_hex_string(
                b"139e6d23bc0a868b9ad2c85e5303faf10cf53f8b006283f1caf7fc3c5972f97",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"4548f399e63675e13bfd344b12336a7e3de2a352b36578876e4461bb829e9ad",
            ),
            field_element_from_hex_string(
                b"64b6dbf861967439f56b2bce22edb4d4e8c90757a29b3d7fa02b86a4ccdbbce",
            ),
            field_element_from_hex_string(
                b"960e47b066c670a206a493f218f4d7322b55238c6a49fd8dabe0a8e91752bb3",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"5210af22881a089f1a281059693dd84035add8e11f906d7af5411ab0cdaaaa3",
            ),
            field_element_from_hex_string(
                b"da22468dd1ab2f2e55c52d9582ac9a58e79b0d187e148b991a84c0cdb836cd6",
            ),
            field_element_from_hex_string(
                b"d04d7d23f4d47f30edc584be44548ec99a4c8684926b69886e58b06f9dbfad3",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"01df81efd8890db4d3aa096d1c4795c1bf9f760a3bda7e1cc8373a2e0e317be",
            ),
            field_element_from_hex_string(
                b"833a1a26a48779724ff5b438b20a79578d76a0189f03c1f886992e7600f4ca9",
            ),
            field_element_from_hex_string(
                b"ae877a8b5122c38b84a7ac864e0d6b8d79843621e5f7e63940a2d3c46cfd31e",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"6b7d5971e48e4a33d841b007c60e53d08a445561bfbf0ff90668220b1be8644",
            ),
            field_element_from_hex_string(
                b"41174878acd1b084e7aa8d08909779eeaa5dee4ee7ae10e3b474d9ed2288fee",
            ),
            field_element_from_hex_string(
                b"0601f43a2a69ae5dba2954493c721a90261f262c13b919dd7aaf4c0e950c247",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"522bb760662a298dbd3020e32b142f9f697408ac41aa62d309f638797fbed90",
            ),
            field_element_from_hex_string(
                b"1567a51295873a63954d47aad4931390da2c469fff06a1d0216bdb490c2109a",
            ),
            field_element_from_hex_string(
                b"1924dac318de31dc500f150a1af687fc8a906a801f2ecc2ccfb53127551d9c6",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"dee613a46fa88a7e1052811e6f1069c9860d853274dfc63e851b894d314b2ac",
            ),
            field_element_from_hex_string(
                b"5424b3bf6ae0f01f1e9f7f780302d1621e5f930d5d56fc648c6e8958f6e5884",
            ),
            field_element_from_hex_string(
                b"ca3a818bb5127377344db60b76b572524485b10e3a2a02f8756c30a683d54bb",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"d996c7896a5ae6193ab48d93103a3032b3634c735d3700f9fde3acf19c39d53",
            ),
            field_element_from_hex_string(
                b"84316711072c7d3117a101b52f840c0e2fba2933c20b7805714c26f48a6645c",
            ),
            field_element_from_hex_string(
                b"8db3000c9d538b6b0425bd7531692b39e8a421f00cf8a95f40892cf98ecfa15",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"0e654dd84ac1c8ff42de4846a6fc35f144767730053b63c08595201f3461efc",
            ),
            field_element_from_hex_string(
                b"64ecf7f6189ae33b609cb3a8bdb15c1282cc20b0c4e17e29dfdb6d249a7c3eb",
            ),
            field_element_from_hex_string(
                b"ca094b1ebc5c5b0af3aaaedff5b7933509399ea88f51462c78d532d24857528",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"016250e7976898f05257813356a213a91eee15a4dfe45cde11ecbf6eb98ac42",
            ),
            field_element_from_hex_string(
                b"989494001bb07726aebf582bc53c5d7bc766aa5bfafa65b3c817731db9e34ad",
            ),
            field_element_from_hex_string(
                b"878e57e025133a531b41f1603765622503e2813f8505ca5a24743ac78ec7db3",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"7e72a20ff2e0e9ac5fd567055606032a6725a7b1f9fa03d88f9c272a8e76b0a",
            ),
            field_element_from_hex_string(
                b"fb6c7dad1c6f5fc0b4e5e81bc5f689dc83476fc7bf785f09f5e6abedde1a729",
            ),
            field_element_from_hex_string(
                b"143e221206777a4dd9a3fd3a9c3af84e2143f5f93090a1be4f202d7cb092835",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"e12bba3da446a914679479bf47c1bd7d95a69ad23372ffc4a51d74009fc94fb",
            ),
            field_element_from_hex_string(
                b"bf06205b17162a41743903320b88606820127118e47de4a1d88c7e33a175873",
            ),
            field_element_from_hex_string(
                b"835e7c593a75bac568c8eee25fce6bc761561c23cf47a90b9c79bb68b38efcf",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"031d610f46a7cd3ffc8ac2a306c13b230d8c7ba66eddc3f4e8a2cb64683faeb",
            ),
            field_element_from_hex_string(
                b"e8976ce2b08d3a09d269636acb5204ee07a5a41a059be55d653390c51c19c8f",
            ),
            field_element_from_hex_string(
                b"42c11e83fc06f194e455a0029b15061e0f71a70cf783e7c4e588fe049580083",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"776ae7fa6c3f375cf2e71ac14c175767d5379a42fa23ca1786dab6592e54805",
            ),
            field_element_from_hex_string(
                b"287d9c271337fd512031249374a4e495f42e238e9b5eaeffc8e3f2605793be2",
            ),
            field_element_from_hex_string(
                b"4f4111a12be7a0ce17920e930b74f93afcd130604dd35ff650dd18cd499a3cd",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"3b67bcaab624645c56d092bb2666ad98d043b2bd46cb1e574552817bc19eefe",
            ),
            field_element_from_hex_string(
                b"31cb61bf19509566bc157725523ad8da7f552243c8ad00f099170d0aa3a626c",
            ),
            field_element_from_hex_string(
                b"baa9ea18c5f8c407611bef69fbe0454d4cf7e6394bbc6839a955999346ab634",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"5f770379c678f84edd0f951159a5537dd4c412a0f27269c9ad24bd76fc31f70",
            ),
            field_element_from_hex_string(
                b"663af090a0ee43a8ca049959e658d5c2a1513ce67966840367a7d8805e0f141",
            ),
            field_element_from_hex_string(
                b"a8bed1088db05b0546e0128a25b8782707f3336379690569783705927a2e33e",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"258ff602d6d41def53eb9847ab9065f0089eed9ebc27848bc9eca4f27102451",
            ),
            field_element_from_hex_string(
                b"403c8d83f36d8c10a50c1b4c3f428f3cee18b70b8887c1410efee6a4188332f",
            ),
            field_element_from_hex_string(
                b"48c0129c7a735617804327e769964439bd8f15c65bce9a6677e7cd50174d4a0",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"83a74c8729f2ae23865d6a7b57aca88709420bdb60afb9613caa0a6b5510a3c",
            ),
            field_element_from_hex_string(
                b"1756a6b6ee8869e883807a1dd6a9bc46d3df55f427fca9f42d4b744f0769ad4",
            ),
            field_element_from_hex_string(
                b"5f2210d44efadbd4defcd1799ffb3c10e7d14328dbe07d5871cf23e96d7d557",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"ec65b71d4688a0a0a6bfb31b1d346b734fd5deba40a94e810b6a4c86c2159ae",
            ),
            field_element_from_hex_string(
                b"8e476cae794bbcc7eacde8e7e3d523268a31f73514924837dbf131795327e59",
            ),
            field_element_from_hex_string(
                b"d1fdf7e186419cfeb6309b4b20b3231ce4ceda35c978d7d315f68be0403e940",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"c6067fe16d92f2c4c59b6caf6074128353de7a3f8b4a083d448856a13c9ddad",
            ),
            field_element_from_hex_string(
                b"b8f3d767c85a5ce718d83e02a5d838c42b5f78bdb4c8c616912c9d559d5e1b6",
            ),
            field_element_from_hex_string(
                b"c914bfcbb0d275a92b1cc2d8a118be6c56e7fdcf8c21a09ff04e41a2daf1670",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"51dfec1ca571d69b96300216aec14086e54798b4acfc170b1b6dc7cc7cd7f68",
            ),
            field_element_from_hex_string(
                b"146277c3ea293a13977ecaa7fd604699ac0929f515e99106f6b420544a724a9",
            ),
            field_element_from_hex_string(
                b"aa1fd46bdf8d4b13c18557010cac1c970060e5dcef6ccadcd83b65d503dcee3",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"14e8beb09dc4b4c6724dcd1f1803977cf4493a0e6242aca78d1bd1322b441f0",
            ),
            field_element_from_hex_string(
                b"eab0ac9f2c6edcfa779368381565734583ace99efbf49bc4d2578f2a57fbe43",
            ),
            field_element_from_hex_string(
                b"86f1c616536dfab561a7bb8f766a615fd91071d090c4482f6d5cdb6b840a6e8",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"4e9d0977b227d8e5c943eb0195213775b926619d3a1fe4f8c39b8428e5f704a",
            ),
            field_element_from_hex_string(
                b"451a8235f05f5909ebbf94bac3b3bc1d9c0130841d5413e24c6c690a17ad557",
            ),
            field_element_from_hex_string(
                b"c4bcd85520f0f11ae6086b7d859ae23eb85f670f1ac0ebd620815b78bc84459",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"76892b48ee0d18ae2407e30a7b8c963fd8449116463c7019ee686c61f04c083",
            ),
            field_element_from_hex_string(
                b"a4d8f7352e99c255edf293c211a2f6801147a7ecc257fe25e19e0c7f923afcc",
            ),
            field_element_from_hex_string(
                b"a21687a3bf64a7e6bf1af7768890d73f748a713c166ff99893218e1ce33a45c",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"e9ce2d27a7da55b2d0b47bedb319a262ed8eea49e3d15883591c7ecb654e579",
            ),
            field_element_from_hex_string(
                b"1af763c8f3cfaa3edc49205e0081be59a41438f0492b2ed519dfa4664f6f6ab",
            ),
            field_element_from_hex_string(
                b"e99515f5dd67e1708c4673cf92b1e92bd81681344c3cec11682c847daa7bbb1",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"56c60fecb812f16abcfd72d1c10d78b1d023c77f525de871c5641f504ab8131",
            ),
            field_element_from_hex_string(
                b"aa0d03d418fdf49529d3564c433488c80f02e82eea8f859aabce122cd8dfff1",
            ),
            field_element_from_hex_string(
                b"c90098a0e1cd2f762e8c8c9d49c5540d6cbf34f637b01e24bd57fdc822614b8",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"d530afad110267b09fc16b83adbf4882b913045b90993f71963bb0343b9f885",
            ),
            field_element_from_hex_string(
                b"30cc7688ec7b4e8b9c9792db5f433d29a46a5302630a86945653e008c20b9f5",
            ),
            field_element_from_hex_string(
                b"4199755b34f7758e7a1029ba1090b8742b9fb9263805fbccc3fb5f9130fedbd",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"8cfccbac32aae8ff190712f5804c17336dc779b7fa1a0e40913550a14b65266",
            ),
            field_element_from_hex_string(
                b"2484d06b2270cbedf7b9ca19f32ca792148c647faaf5c5e96fc29a6cd3885ce",
            ),
            field_element_from_hex_string(
                b"bf0c50a3e542256ca345f8e315e4aea3f6187be1eb8310678e071f518627612",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"8a47086590de274e19d343c65dd88257f364fbc6aa218a0cd070ef5ef1fc177",
            ),
            field_element_from_hex_string(
                b"ebb89884eb16577f110088e41993bf6fc0b187d1375caa694e71ce292d19576",
            ),
            field_element_from_hex_string(
                b"d8aaaa757b27e6ab1960494e096079d14e800098663b720d87580151ef1af1d",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"c1df45f9debb755c8643917ca72c0de0ba93578f7bd8213dd9cefae75731653",
            ),
            field_element_from_hex_string(
                b"2e427f22b17a763503edc3de0a51c72d0a3817032fa96edc4ed0839f1383dfe",
            ),
            field_element_from_hex_string(
                b"8ffbbedb45f34af59ed72f7bd330f9a23cd95a21c3df255a9b4bc7e03672cf9",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"5aefb65a230cb9adfda29ca7967909fc9f99abb9a028ffa2af8193db8c4cc18",
            ),
            field_element_from_hex_string(
                b"bb48330fb73b7f787b497f84086d130a1254e16c93818ac28208549fc904cd5",
            ),
            field_element_from_hex_string(
                b"12202f2312070a9f1bbce26672fb483ca57c6dd48b48abb6bf7aa7261e56b80",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"e5dd6c10a0eefebdb6b0cf1416cd350d02687348b66afe980d12a8c630da031",
            ),
            field_element_from_hex_string(
                b"3f7f9fd42d66b2f0874efdb148ec72d392b671dcb16dd3014f8773a531258da",
            ),
            field_element_from_hex_string(
                b"ee8f260a6bedd569aa7d8a3760c5998d5f657b7843e84914936bef7e8cf698b",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"f265e22acb876b3658cb94f8b97a1039c0eb481bd00bfaa235eaadc44b43f4a",
            ),
            field_element_from_hex_string(
                b"1332c2e57955b010272e3ea72aeaec80d7bff5c2662d9786b898d9eb340679e",
            ),
            field_element_from_hex_string(
                b"bfdfe01b8376fc8a5600eb7947be731d93d4c3ed5ae23e98bb646190ef2d939",
            ),
        ],
        vec![
            field_element_from_hex_string(
                b"fd787aaa0cd48f4856d56196856f1f4da588bce6894975f936b0c5a0099ae70",
            ),
            field_element_from_hex_string(
                b"3c20716b1ea1d0a507f4332deea9fa45af81af0ffe6b44fd758c6b3e9ac85f4",
            ),
            field_element_from_hex_string(
                b"dbd5e508127c35a9a6deac0cd342af4395ae60ee6343476be72f59cf0b4377d",
            ),
        ],
    ]
}

/// Converts a literal hexadecimal string to a field element through BigUint
/// this function should only ever be called on the constants above, so we panic
/// if parsing fails
fn field_element_from_hex_string(byte_string: &[u8]) -> DalekRistrettoField {
    DalekRistrettoField::from(BigUint::parse_bytes(byte_string, 16 /* radix */).unwrap())
}

#[cfg(test)]
mod test {
    use super::{POSEIDON_MDS_MATRIX_T_3, POSEIDON_ROUND_CONSTANTS_T_3};

    #[test]
    fn test_parsing() {
        // Does not panic during parse
        POSEIDON_MDS_MATRIX_T_3();
        POSEIDON_ROUND_CONSTANTS_T_3();
    }
}
