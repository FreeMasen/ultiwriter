//! Thread charts used by formats that store palette indices rather than RGB.
//! Generated from pyembroidery 1.5.1 (MIT) `EmbThreadPec` / `EmbThreadJef`.

pub struct ChartThread {
    pub rgb: u32,
    pub name: &'static str,
    pub code: &'static str,
}

pub fn rgbs(chart: &[ChartThread]) -> Vec<u32> {
    chart.iter().map(|t| t.rgb).collect()
}

/// Brother PEC thread chart (PES/PEC colour indices).
/// Index 0 is unused (`None` in pyembroidery).
pub const PEC_THREADS: &[ChartThread] = &[
    ChartThread {
        rgb: 0x000000,
        name: "",
        code: "",
    },
    ChartThread {
        rgb: 0x0E1F7C,
        name: "Prussian Blue",
        code: "1",
    },
    ChartThread {
        rgb: 0x0A55A3,
        name: "Blue",
        code: "2",
    },
    ChartThread {
        rgb: 0x008777,
        name: "Teal Green",
        code: "3",
    },
    ChartThread {
        rgb: 0x4B6BAF,
        name: "Cornflower Blue",
        code: "4",
    },
    ChartThread {
        rgb: 0xED171F,
        name: "Red",
        code: "5",
    },
    ChartThread {
        rgb: 0xD15C00,
        name: "Reddish Brown",
        code: "6",
    },
    ChartThread {
        rgb: 0x913697,
        name: "Magenta",
        code: "7",
    },
    ChartThread {
        rgb: 0xE49ACB,
        name: "Light Lilac",
        code: "8",
    },
    ChartThread {
        rgb: 0x915FAC,
        name: "Lilac",
        code: "9",
    },
    ChartThread {
        rgb: 0x9ED67D,
        name: "Mint Green",
        code: "10",
    },
    ChartThread {
        rgb: 0xE8A900,
        name: "Deep Gold",
        code: "11",
    },
    ChartThread {
        rgb: 0xFEBA35,
        name: "Orange",
        code: "12",
    },
    ChartThread {
        rgb: 0xFFFF00,
        name: "Yellow",
        code: "13",
    },
    ChartThread {
        rgb: 0x70BC1F,
        name: "Lime Green",
        code: "14",
    },
    ChartThread {
        rgb: 0xBA9800,
        name: "Brass",
        code: "15",
    },
    ChartThread {
        rgb: 0xA8A8A8,
        name: "Silver",
        code: "16",
    },
    ChartThread {
        rgb: 0x7D6F00,
        name: "Russet Brown",
        code: "17",
    },
    ChartThread {
        rgb: 0xFFFFB3,
        name: "Cream Brown",
        code: "18",
    },
    ChartThread {
        rgb: 0x4F5556,
        name: "Pewter",
        code: "19",
    },
    ChartThread {
        rgb: 0x000000,
        name: "Black",
        code: "20",
    },
    ChartThread {
        rgb: 0x0B3D91,
        name: "Ultramarine",
        code: "21",
    },
    ChartThread {
        rgb: 0x770176,
        name: "Royal Purple",
        code: "22",
    },
    ChartThread {
        rgb: 0x293133,
        name: "Dark Gray",
        code: "23",
    },
    ChartThread {
        rgb: 0x2A1301,
        name: "Dark Brown",
        code: "24",
    },
    ChartThread {
        rgb: 0xF64A8A,
        name: "Deep Rose",
        code: "25",
    },
    ChartThread {
        rgb: 0xB27624,
        name: "Light Brown",
        code: "26",
    },
    ChartThread {
        rgb: 0xFCBBC5,
        name: "Salmon Pink",
        code: "27",
    },
    ChartThread {
        rgb: 0xFE370F,
        name: "Vermilion",
        code: "28",
    },
    ChartThread {
        rgb: 0xF0F0F0,
        name: "White",
        code: "29",
    },
    ChartThread {
        rgb: 0x6A1C8A,
        name: "Violet",
        code: "30",
    },
    ChartThread {
        rgb: 0xA8DDC4,
        name: "Seacrest",
        code: "31",
    },
    ChartThread {
        rgb: 0x2584BB,
        name: "Sky Blue",
        code: "32",
    },
    ChartThread {
        rgb: 0xFEB343,
        name: "Pumpkin",
        code: "33",
    },
    ChartThread {
        rgb: 0xFFF36B,
        name: "Cream Yellow",
        code: "34",
    },
    ChartThread {
        rgb: 0xD0A660,
        name: "Khaki",
        code: "35",
    },
    ChartThread {
        rgb: 0xD15400,
        name: "Clay Brown",
        code: "36",
    },
    ChartThread {
        rgb: 0x66BA49,
        name: "Leaf Green",
        code: "37",
    },
    ChartThread {
        rgb: 0x134A46,
        name: "Peacock Blue",
        code: "38",
    },
    ChartThread {
        rgb: 0x878787,
        name: "Gray",
        code: "39",
    },
    ChartThread {
        rgb: 0xD8CCC6,
        name: "Warm Gray",
        code: "40",
    },
    ChartThread {
        rgb: 0x435607,
        name: "Dark Olive",
        code: "41",
    },
    ChartThread {
        rgb: 0xFDD9DE,
        name: "Flesh Pink",
        code: "42",
    },
    ChartThread {
        rgb: 0xF993BC,
        name: "Pink",
        code: "43",
    },
    ChartThread {
        rgb: 0x003822,
        name: "Deep Green",
        code: "44",
    },
    ChartThread {
        rgb: 0xB2AFD4,
        name: "Lavender",
        code: "45",
    },
    ChartThread {
        rgb: 0x686AB0,
        name: "Wisteria Violet",
        code: "46",
    },
    ChartThread {
        rgb: 0xEFE3B9,
        name: "Beige",
        code: "47",
    },
    ChartThread {
        rgb: 0xF73866,
        name: "Carmine",
        code: "48",
    },
    ChartThread {
        rgb: 0xB54B64,
        name: "Amber Red",
        code: "49",
    },
    ChartThread {
        rgb: 0x132B1A,
        name: "Olive Green",
        code: "50",
    },
    ChartThread {
        rgb: 0xC70156,
        name: "Dark Fuchsia",
        code: "51",
    },
    ChartThread {
        rgb: 0xFE9E32,
        name: "Tangerine",
        code: "52",
    },
    ChartThread {
        rgb: 0xA8DEEB,
        name: "Light Blue",
        code: "53",
    },
    ChartThread {
        rgb: 0x00673E,
        name: "Emerald Green",
        code: "54",
    },
    ChartThread {
        rgb: 0x4E2990,
        name: "Purple",
        code: "55",
    },
    ChartThread {
        rgb: 0x2F7E20,
        name: "Moss Green",
        code: "56",
    },
    ChartThread {
        rgb: 0xFFCCCC,
        name: "Flesh Pink",
        code: "57",
    },
    ChartThread {
        rgb: 0xFFD911,
        name: "Harvest Gold",
        code: "58",
    },
    ChartThread {
        rgb: 0x095BA6,
        name: "Electric Blue",
        code: "59",
    },
    ChartThread {
        rgb: 0xF0F970,
        name: "Lemon Yellow",
        code: "60",
    },
    ChartThread {
        rgb: 0xE3F35B,
        name: "Fresh Green",
        code: "61",
    },
    ChartThread {
        rgb: 0xFF9900,
        name: "Orange",
        code: "62",
    },
    ChartThread {
        rgb: 0xFFF08D,
        name: "Cream Yellow",
        code: "63",
    },
    ChartThread {
        rgb: 0xFFC8C8,
        name: "Applique",
        code: "64",
    },
];

/// Janome JEF thread chart (JEF colour indices).
/// Index 0 is unused (`None` in pyembroidery).
pub const JEF_THREADS: &[ChartThread] = &[
    ChartThread {
        rgb: 0x000000,
        name: "",
        code: "",
    },
    ChartThread {
        rgb: 0x000000,
        name: "Black",
        code: "002",
    },
    ChartThread {
        rgb: 0xFFFFFF,
        name: "White",
        code: "001",
    },
    ChartThread {
        rgb: 0xFFFF17,
        name: "Yellow",
        code: "204",
    },
    ChartThread {
        rgb: 0xFF6600,
        name: "Orange",
        code: "203",
    },
    ChartThread {
        rgb: 0x2F5933,
        name: "Olive Green",
        code: "219",
    },
    ChartThread {
        rgb: 0x237336,
        name: "Green",
        code: "226",
    },
    ChartThread {
        rgb: 0x65C2C8,
        name: "Sky",
        code: "217",
    },
    ChartThread {
        rgb: 0xAB5A96,
        name: "Purple",
        code: "208",
    },
    ChartThread {
        rgb: 0xF669A0,
        name: "Pink",
        code: "201",
    },
    ChartThread {
        rgb: 0xFF0000,
        name: "Red",
        code: "225",
    },
    ChartThread {
        rgb: 0xB1704E,
        name: "Brown",
        code: "214",
    },
    ChartThread {
        rgb: 0x0B2F84,
        name: "Blue",
        code: "207",
    },
    ChartThread {
        rgb: 0xE4C35D,
        name: "Gold",
        code: "003",
    },
    ChartThread {
        rgb: 0x481A05,
        name: "Dark Brown",
        code: "205",
    },
    ChartThread {
        rgb: 0xAC9CC7,
        name: "Pale Violet",
        code: "209",
    },
    ChartThread {
        rgb: 0xFCF294,
        name: "Pale Yellow",
        code: "210",
    },
    ChartThread {
        rgb: 0xF999B7,
        name: "Pale Pink",
        code: "211",
    },
    ChartThread {
        rgb: 0xFAB381,
        name: "Peach",
        code: "212",
    },
    ChartThread {
        rgb: 0xC9A480,
        name: "Beige",
        code: "213",
    },
    ChartThread {
        rgb: 0x970533,
        name: "Wine Red",
        code: "215",
    },
    ChartThread {
        rgb: 0xA0B8CC,
        name: "Pale Sky",
        code: "216",
    },
    ChartThread {
        rgb: 0x7FC21C,
        name: "Yellow Green",
        code: "218",
    },
    ChartThread {
        rgb: 0xE5E5E5,
        name: "Silver Gray",
        code: "220",
    },
    ChartThread {
        rgb: 0x889B9B,
        name: "Gray",
        code: "221",
    },
    ChartThread {
        rgb: 0x98D6BD,
        name: "Pale Aqua",
        code: "227",
    },
    ChartThread {
        rgb: 0xB2E1E3,
        name: "Baby Blue",
        code: "228",
    },
    ChartThread {
        rgb: 0x368BA0,
        name: "Powder Blue",
        code: "229",
    },
    ChartThread {
        rgb: 0x4F83AB,
        name: "Bright Blue",
        code: "230",
    },
    ChartThread {
        rgb: 0x386A91,
        name: "Slate Blue",
        code: "231",
    },
    ChartThread {
        rgb: 0x071650,
        name: "Navy Blue",
        code: "232",
    },
    ChartThread {
        rgb: 0xF999A2,
        name: "Salmon Pink",
        code: "233",
    },
    ChartThread {
        rgb: 0xF9676B,
        name: "Coral",
        code: "234",
    },
    ChartThread {
        rgb: 0xE3311F,
        name: "Burnt Orange",
        code: "235",
    },
    ChartThread {
        rgb: 0xE2A188,
        name: "Cinnamon",
        code: "236",
    },
    ChartThread {
        rgb: 0xB59474,
        name: "Umber",
        code: "237",
    },
    ChartThread {
        rgb: 0xE4CF99,
        name: "Blond",
        code: "238",
    },
    ChartThread {
        rgb: 0xFFCB00,
        name: "Sunflower",
        code: "239",
    },
    ChartThread {
        rgb: 0xE1ADD4,
        name: "Orchid Pink",
        code: "240",
    },
    ChartThread {
        rgb: 0xC3007E,
        name: "Peony Purple",
        code: "241",
    },
    ChartThread {
        rgb: 0x80004B,
        name: "Burgundy",
        code: "242",
    },
    ChartThread {
        rgb: 0x540571,
        name: "Royal Purple",
        code: "243",
    },
    ChartThread {
        rgb: 0xB10525,
        name: "Cardinal Red",
        code: "244",
    },
    ChartThread {
        rgb: 0xCAE0C0,
        name: "Opal Green",
        code: "245",
    },
    ChartThread {
        rgb: 0x899856,
        name: "Moss Green",
        code: "246",
    },
    ChartThread {
        rgb: 0x5C941A,
        name: "Meadow Green",
        code: "247",
    },
    ChartThread {
        rgb: 0x003114,
        name: "Dark Green",
        code: "248",
    },
    ChartThread {
        rgb: 0x5DAE94,
        name: "Aquamarine",
        code: "249",
    },
    ChartThread {
        rgb: 0x4CBF8F,
        name: "Emerald Green",
        code: "250",
    },
    ChartThread {
        rgb: 0x007772,
        name: "Peacock Green",
        code: "251",
    },
    ChartThread {
        rgb: 0x595B61,
        name: "Dark Gray",
        code: "252",
    },
    ChartThread {
        rgb: 0xFFFFF2,
        name: "Ivory White",
        code: "253",
    },
    ChartThread {
        rgb: 0xB15818,
        name: "Hazel",
        code: "254",
    },
    ChartThread {
        rgb: 0xCB8A07,
        name: "Toast",
        code: "255",
    },
    ChartThread {
        rgb: 0x986C80,
        name: "Salmon",
        code: "256",
    },
    ChartThread {
        rgb: 0x98692D,
        name: "Cocoa Brown",
        code: "257",
    },
    ChartThread {
        rgb: 0x4D3419,
        name: "Sienna",
        code: "258",
    },
    ChartThread {
        rgb: 0x4C330B,
        name: "Sepia",
        code: "259",
    },
    ChartThread {
        rgb: 0x33200A,
        name: "Dark Sepia",
        code: "260",
    },
    ChartThread {
        rgb: 0x523A97,
        name: "Violet Blue",
        code: "261",
    },
    ChartThread {
        rgb: 0x0D217E,
        name: "Blue Ink",
        code: "262",
    },
    ChartThread {
        rgb: 0x1E77AC,
        name: "Sola Blue",
        code: "263",
    },
    ChartThread {
        rgb: 0xB2DD53,
        name: "Green Dust",
        code: "264",
    },
    ChartThread {
        rgb: 0xF33689,
        name: "Crimson",
        code: "265",
    },
    ChartThread {
        rgb: 0xDE649E,
        name: "Floral Pink",
        code: "266",
    },
    ChartThread {
        rgb: 0x984161,
        name: "Wine",
        code: "267",
    },
    ChartThread {
        rgb: 0x4C5612,
        name: "Olive Drab",
        code: "268",
    },
    ChartThread {
        rgb: 0x4C881F,
        name: "Meadow",
        code: "269",
    },
    ChartThread {
        rgb: 0xE4DE79,
        name: "Mustard",
        code: "270",
    },
    ChartThread {
        rgb: 0xCB8A1A,
        name: "Yellow Ocher",
        code: "271",
    },
    ChartThread {
        rgb: 0xCBA21C,
        name: "Old Gold",
        code: "272",
    },
    ChartThread {
        rgb: 0xFF9805,
        name: "Honey Dew",
        code: "273",
    },
    ChartThread {
        rgb: 0xFCB257,
        name: "Tangerine",
        code: "274",
    },
    ChartThread {
        rgb: 0xFFE505,
        name: "Canary Yellow",
        code: "275",
    },
    ChartThread {
        rgb: 0xF0331F,
        name: "Vermilion",
        code: "202",
    },
    ChartThread {
        rgb: 0x1A842D,
        name: "Bright Green",
        code: "206",
    },
    ChartThread {
        rgb: 0x386CAE,
        name: "Ocean Blue",
        code: "222",
    },
    ChartThread {
        rgb: 0xE3C4B4,
        name: "Beige Gray",
        code: "223",
    },
    ChartThread {
        rgb: 0xE3AC81,
        name: "Bamboo",
        code: "224",
    },
];
