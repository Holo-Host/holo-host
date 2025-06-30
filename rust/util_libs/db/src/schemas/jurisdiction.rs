use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use utoipa::{openapi, PartialSchema, ToSchema};

#[derive(
    Serialize, Deserialize, EnumString, Display, Debug, Clone, PartialEq, Eq, Hash, Default,
)]
#[strum(serialize_all = "title_case")]
pub enum Jurisdiction {
    #[serde(rename = "Unknown")]
    #[default]
    Unknown,
    #[serde(rename = "Afghanistan")]
    Afghanistan,
    #[serde(rename = "Aland Islands")]
    AlandIslands,
    #[serde(rename = "Albania")]
    Albania,
    #[serde(rename = "Algeria")]
    Algeria,
    #[serde(rename = "Andorra")]
    Andorra,
    #[serde(rename = "Angola")]
    Angola,
    #[serde(rename = "Anguilla")]
    Anguilla,
    #[serde(rename = "Antigua And Barbuda")]
    AntiguaAndBarbuda,
    #[serde(rename = "Argentina")]
    Argentina,
    #[serde(rename = "Armenia")]
    Armenia,
    #[serde(rename = "Aruba")]
    Aruba,
    #[serde(rename = "Australia")]
    Australia,
    #[serde(rename = "Austria")]
    Austria,
    #[serde(rename = "Azerbaijan")]
    Azerbaijan,
    #[serde(rename = "Bahamas")]
    Bahamas,
    #[serde(rename = "Bahrain")]
    Bahrain,
    #[serde(rename = "Bangladesh")]
    Bangladesh,
    #[serde(rename = "Barbados")]
    Barbados,
    #[serde(rename = "Belarus")]
    Belarus,
    #[serde(rename = "Belgium")]
    Belgium,
    #[serde(rename = "Belize")]
    Belize,
    #[serde(rename = "Benin")]
    Benin,
    #[serde(rename = "Bermuda")]
    Bermuda,
    #[serde(rename = "Bhutan")]
    Bhutan,
    #[serde(rename = "Bolivia")]
    Bolivia,
    #[serde(rename = "Bosnia And Herzegovina")]
    BosniaAndHerzegovina,
    #[serde(rename = "Botswana")]
    Botswana,
    #[serde(rename = "Bouvet Island")]
    BouvetIsland,
    #[serde(rename = "Brazil")]
    Brazil,
    #[serde(rename = "British Indian Ocean Territory")]
    BritishIndianOceanTerritory,
    #[serde(rename = "Virgin Islands, British")]
    VirginIslandsBritish,
    #[serde(rename = "Brunei")]
    Brunei,
    #[serde(rename = "Bulgaria")]
    Bulgaria,
    #[serde(rename = "Burkina Faso")]
    BurkinaFaso,
    #[serde(rename = "Burundi")]
    Burundi,
    #[serde(rename = "Cambodia")]
    Cambodia,
    #[serde(rename = "Republic of Cameroon")]
    RepublicOfCameroon,
    #[serde(rename = "Canada")]
    Canada,
    #[serde(rename = "Cape Verde")]
    CapeVerde,
    #[serde(rename = "Caribbean Netherlands")]
    CaribbeanNetherlands,
    #[serde(rename = "Cayman Islands")]
    CaymanIslands,
    #[serde(rename = "Central African Republic")]
    CentralAfricanRepublic,
    #[serde(rename = "Chad")]
    Chad,
    #[serde(rename = "Chile")]
    Chile,
    #[serde(rename = "China")]
    China,
    #[serde(rename = "Christmas Island")]
    ChristmasIsland,
    #[serde(rename = "Cocos (Keeling) Islands")]
    CocosKeelingIslands,
    #[serde(rename = "Colombia")]
    Colombia,
    #[serde(rename = "Comoros")]
    Comoros,
    #[serde(rename = "Congo")]
    Congo,
    #[serde(rename = "Congo, The Democratic Republic Of The")]
    CongoTheDemocraticRepublicOfThe,
    #[serde(rename = "Cook Islands")]
    CookIslands,
    #[serde(rename = "Costa Rica")]
    CostaRica,
    #[serde(rename = "Croatia")]
    Croatia,
    #[serde(rename = "Curaçao")]
    Curacao,
    #[serde(rename = "Cyprus")]
    Cyprus,
    #[serde(rename = "Czech Republic")]
    CzechRepublic,
    #[serde(rename = "Côte d'Ivoire")]
    CoteDIvoire,
    #[serde(rename = "Denmark")]
    Denmark,
    #[serde(rename = "Djibouti")]
    Djibouti,
    #[serde(rename = "Dominica")]
    Dominica,
    #[serde(rename = "Dominican Republic")]
    DominicanRepublic,
    #[serde(rename = "Ecuador")]
    Ecuador,
    #[serde(rename = "Egypt")]
    Egypt,
    #[serde(rename = "El Salvador")]
    ElSalvador,
    #[serde(rename = "Equatorial Guinea")]
    EquatorialGuinea,
    #[serde(rename = "Eritrea")]
    Eritrea,
    #[serde(rename = "Estonia")]
    Estonia,
    #[serde(rename = "Eswatini")]
    Eswatini,
    #[serde(rename = "Ethiopia")]
    Ethiopia,
    #[serde(rename = "Falkland Islands (Malvinas)")]
    FalklandIslandsMalvinas,
    #[serde(rename = "Faroe Islands")]
    FaroeIslands,
    #[serde(rename = "Fiji")]
    Fiji,
    #[serde(rename = "Finland")]
    Finland,
    #[serde(rename = "France")]
    France,
    #[serde(rename = "French Guiana")]
    FrenchGuiana,
    #[serde(rename = "French Polynesia")]
    FrenchPolynesia,
    #[serde(rename = "French Southern Territories")]
    FrenchSouthernTerritories,
    #[serde(rename = "Gabon")]
    Gabon,
    #[serde(rename = "Gambia")]
    Gambia,
    #[serde(rename = "Georgia")]
    Georgia,
    #[serde(rename = "Germany")]
    Germany,
    #[serde(rename = "Ghana")]
    Ghana,
    #[serde(rename = "Gibraltar")]
    Gibraltar,
    #[serde(rename = "Greece")]
    Greece,
    #[serde(rename = "Greenland")]
    Greenland,
    #[serde(rename = "Grenada")]
    Grenada,
    #[serde(rename = "Guadeloupe")]
    Guadeloupe,
    #[serde(rename = "Guatemala")]
    Guatemala,
    #[serde(rename = "Guernsey")]
    Guernsey,
    #[serde(rename = "Guinea")]
    Guinea,
    #[serde(rename = "Guinea Bissau")]
    GuineaBissau,
    #[serde(rename = "Guyana")]
    Guyana,
    #[serde(rename = "Haiti")]
    Haiti,
    #[serde(rename = "Heard Island And Mcdonald Islands")]
    HeardIslandAndMcdonaldIslands,
    #[serde(rename = "Honduras")]
    Honduras,
    #[serde(rename = "Hong Kong")]
    HongKong,
    #[serde(rename = "Hungary")]
    Hungary,
    #[serde(rename = "Iceland")]
    Iceland,
    #[serde(rename = "India")]
    India,
    #[serde(rename = "Indonesia")]
    Indonesia,
    #[serde(rename = "Ireland")]
    Ireland,
    #[serde(rename = "Isle Of Man")]
    IsleOfMan,
    #[serde(rename = "Israel")]
    Israel,
    #[serde(rename = "Italy")]
    Italy,
    #[serde(rename = "Jamaica")]
    Jamaica,
    #[serde(rename = "Japan")]
    Japan,
    #[serde(rename = "Jersey")]
    Jersey,
    #[serde(rename = "Jordan")]
    Jordan,
    #[serde(rename = "Kazakhstan")]
    Kazakhstan,
    #[serde(rename = "Kenya")]
    Kenya,
    #[serde(rename = "Kiribati")]
    Kiribati,
    #[serde(rename = "Kosovo")]
    Kosovo,
    #[serde(rename = "Kuwait")]
    Kuwait,
    #[serde(rename = "Kyrgyzstan")]
    Kyrgyzstan,
    #[serde(rename = "Lao Peoples Democratic Republic")]
    LaoPeoplesDemocraticRepublic,
    #[serde(rename = "Latvia")]
    Latvia,
    #[serde(rename = "Lebanon")]
    Lebanon,
    #[serde(rename = "Lesotho")]
    Lesotho,
    #[serde(rename = "Liberia")]
    Liberia,
    #[serde(rename = "Libyan Arab Jamahiriya")]
    LibyanArabJamahiriya,
    #[serde(rename = "Liechtenstein")]
    Liechtenstein,
    #[serde(rename = "Lithuania")]
    Lithuania,
    #[serde(rename = "Luxembourg")]
    Luxembourg,
    #[serde(rename = "Macao")]
    Macao,
    #[serde(rename = "Madagascar")]
    Madagascar,
    #[serde(rename = "Malawi")]
    Malawi,
    #[serde(rename = "Malaysia")]
    Malaysia,
    #[serde(rename = "Maldives")]
    Maldives,
    #[serde(rename = "Mali")]
    Mali,
    #[serde(rename = "Malta")]
    Malta,
    #[serde(rename = "Martinique")]
    Martinique,
    #[serde(rename = "Mauritania")]
    Mauritania,
    #[serde(rename = "Mauritius")]
    Mauritius,
    #[serde(rename = "Mayotte")]
    Mayotte,
    #[serde(rename = "Mexico")]
    Mexico,
    #[serde(rename = "Moldova, Republic of")]
    MoldovaRepublicOf,
    #[serde(rename = "Monaco")]
    Monaco,
    #[serde(rename = "Mongolia")]
    Mongolia,
    #[serde(rename = "Montenegro")]
    Montenegro,
    #[serde(rename = "Montserrat")]
    Montserrat,
    #[serde(rename = "Morocco")]
    Morocco,
    #[serde(rename = "Mozambique")]
    Mozambique,
    #[serde(rename = "Myanmar")]
    Myanmar,
    #[serde(rename = "Namibia")]
    Namibia,
    #[serde(rename = "Nauru")]
    Nauru,
    #[serde(rename = "Nepal")]
    Nepal,
    #[serde(rename = "Netherlands")]
    Netherlands,
    #[serde(rename = "Netherlands Antilles")]
    NetherlandsAntilles,
    #[serde(rename = "New Caledonia")]
    NewCaledonia,
    #[serde(rename = "New Zealand")]
    NewZealand,
    #[serde(rename = "Nicaragua")]
    Nicaragua,
    #[serde(rename = "Niger")]
    Niger,
    #[serde(rename = "Nigeria")]
    Nigeria,
    #[serde(rename = "Niue")]
    Niue,
    #[serde(rename = "Norfolk Island")]
    NorfolkIsland,
    #[serde(rename = "North Macedonia")]
    NorthMacedonia,
    #[serde(rename = "Norway")]
    Norway,
    #[serde(rename = "Oman")]
    Oman,
    #[serde(rename = "Pakistan")]
    Pakistan,
    #[serde(rename = "Palestine, State of")]
    PalestineStateOf,
    #[serde(rename = "Panama")]
    Panama,
    #[serde(rename = "Papua New Guinea")]
    PapuaNewGuinea,
    #[serde(rename = "Paraguay")]
    Paraguay,
    #[serde(rename = "Peru")]
    Peru,
    #[serde(rename = "Philippines")]
    Philippines,
    #[serde(rename = "Pitcairn")]
    Pitcairn,
    #[serde(rename = "Poland")]
    Poland,
    #[serde(rename = "Portugal")]
    Portugal,
    #[serde(rename = "Qatar")]
    Qatar,
    #[serde(rename = "Reunion")]
    Reunion,
    #[serde(rename = "Romania")]
    Romania,
    #[serde(rename = "Russia")]
    Russia,
    #[serde(rename = "Rwanda")]
    Rwanda,
    #[serde(rename = "Samoa")]
    Samoa,
    #[serde(rename = "San Marino")]
    SanMarino,
    #[serde(rename = "Sao Tome And Principe")]
    SaoTomeAndPrincipe,
    #[serde(rename = "Saudi Arabia")]
    SaudiArabia,
    #[serde(rename = "Senegal")]
    Senegal,
    #[serde(rename = "Serbia")]
    Serbia,
    #[serde(rename = "Seychelles")]
    Seychelles,
    #[serde(rename = "Sierra Leone")]
    SierraLeone,
    #[serde(rename = "Singapore")]
    Singapore,
    #[serde(rename = "Sint Maarten")]
    SintMaarten,
    #[serde(rename = "Slovakia")]
    Slovakia,
    #[serde(rename = "Slovenia")]
    Slovenia,
    #[serde(rename = "Solomon Islands")]
    SolomonIslands,
    #[serde(rename = "Somalia")]
    Somalia,
    #[serde(rename = "South Africa")]
    SouthAfrica,
    #[serde(rename = "South Georgia And The South Sandwich Islands")]
    SouthGeorgiaAndTheSouthSandwichIslands,
    #[serde(rename = "South Korea")]
    SouthKorea,
    #[serde(rename = "South Sudan")]
    SouthSudan,
    #[serde(rename = "Spain")]
    Spain,
    #[serde(rename = "Sri Lanka")]
    SriLanka,
    #[serde(rename = "Saint Barthélemy")]
    SaintBarthelemy,
    #[serde(rename = "Saint Helena")]
    SaintHelena,
    #[serde(rename = "Saint Kitts And Nevis")]
    SaintKittsAndNevis,
    #[serde(rename = "Saint Lucia")]
    SaintLucia,
    #[serde(rename = "Saint Martin")]
    SaintMartin,
    #[serde(rename = "Saint Pierre And Miquelon")]
    SaintPierreAndMiquelon,
    #[serde(rename = "St. Vincent")]
    StVincent,
    #[serde(rename = "Suriname")]
    Suriname,
    #[serde(rename = "Svalbard And Jan Mayen")]
    SvalbardAndJanMayen,
    #[serde(rename = "Sweden")]
    Sweden,
    #[serde(rename = "Switzerland")]
    Switzerland,
    #[serde(rename = "Taiwan (Province of China)")]
    TaiwanProvinceOfChina,
    #[serde(rename = "Tajikistan")]
    Tajikistan,
    #[serde(rename = "Tanzania, United Republic Of")]
    TanzaniaUnitedRepublicOf,
    #[serde(rename = "Thailand")]
    Thailand,
    #[serde(rename = "Timor Leste")]
    TimorLeste,
    #[serde(rename = "Togo")]
    Togo,
    #[serde(rename = "Tokelau")]
    Tokelau,
    #[serde(rename = "Tonga")]
    Tonga,
    #[serde(rename = "Trinidad and Tobago")]
    TrinidadAndTobago,
    #[serde(rename = "Tunisia")]
    Tunisia,
    #[serde(rename = "Turkey")]
    Turkey,
    #[serde(rename = "Turkmenistan")]
    Turkmenistan,
    #[serde(rename = "Turks and Caicos Islands")]
    TurksAndCaicosIslands,
    #[serde(rename = "Tuvalu")]
    Tuvalu,
    #[serde(rename = "United States Minor Outlying Islands")]
    UnitedStatesMinorOutlyingIslands,
    #[serde(rename = "Uganda")]
    Uganda,
    #[serde(rename = "Ukraine")]
    Ukraine,
    #[serde(rename = "United Arab Emirates")]
    UnitedArabEmirates,
    #[serde(rename = "United Kingdom")]
    UnitedKingdom,
    #[serde(rename = "United States")]
    UnitedStates,
    #[serde(rename = "Uruguay")]
    Uruguay,
    #[serde(rename = "Uzbekistan")]
    Uzbekistan,
    #[serde(rename = "Vanuatu")]
    Vanuatu,
    #[serde(rename = "Holy See (Vatican City State)")]
    HolySeeVaticanCityState,
    #[serde(rename = "Venezuela")]
    Venezuela,
    #[serde(rename = "Vietnam")]
    Vietnam,
    #[serde(rename = "Wallis And Futuna")]
    WallisAndFutuna,
    #[serde(rename = "Western Sahara")]
    WesternSahara,
    #[serde(rename = "Yemen")]
    Yemen,
    #[serde(rename = "Zambia")]
    Zambia,
    #[serde(rename = "Zimbabwe")]
    Zimbabwe,
}

impl PartialSchema for Jurisdiction {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        let schema = openapi::schema::Object::builder()
            .schema_type(openapi::schema::SchemaType::Type(
                openapi::schema::Type::Object,
            ))
            .title(Some("Jurisdiction".to_string()))
            .examples(vec![Jurisdiction::Unknown.to_string()])
            .build();

        openapi::RefOr::T(openapi::schema::Schema::Object(schema))
    }
}
impl ToSchema for Jurisdiction {}
