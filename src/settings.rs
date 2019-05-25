use config::{Config, Environment, File, FileFormat};
use failure::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DestDrive {
    pub label: String,
    pub format: String,
}

impl Default for DestDrive {
    fn default() -> Self {
        DestDrive {
            label: "$CURRENTDRIVE".to_owned(),
            format: "$HOSTNAME/".to_owned(),
        }
    }
}

fn lower_all<'de, D>(t: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Vec::<String>::deserialize(t)?
        .into_iter()
        .map(|x| x.to_lowercase())
        .collect())
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Match {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "lower_all")]
    pub exclude: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "lower_all")]
    pub only: Vec<String>,
}

impl Default for Match {
    fn default() -> Self {
        Match {
            exclude: vec![],
            only: vec![],
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SrcFile {
    pub from: String,
    pub to: String,
    #[serde(default = "Vec::new")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<Match>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub dryrun: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            quiet: false,
            dryrun: true,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    #[serde(default)]
    pub config: AppConfig,
    #[serde(default)]
    pub dest: DestDrive,
    #[serde(default)]
    pub files: Vec<SrcFile>,
}

impl Default for Settings {
    fn default() -> Self {
        let no_public_user = Match {
            exclude: vec![
                "All Users".to_owned(),
                "Default".to_owned(),
                "Default User".to_owned(),
                "DefaultAppPool".to_owned(),
                "Public".to_owned(),
            ],
            only: vec![],
        };

        Settings {
            config: AppConfig::default(),
            dest: DestDrive::default(),
            files: vec![
                SrcFile {
                    from: "C:\\Users\\*\\*\\".to_owned(),
                    to: "$1/$2/".to_owned(),
                    filters: vec![
                        no_public_user.clone(),
                        Match {
                            exclude: Vec::new(),
                            only: vec![
                                "Desktop".to_owned(),
                                "Downloads".to_owned(),
                                "Contacts".to_owned(),
                            ],
                        },
                    ],
                },
                SrcFile {
                    from: "C:\\Users\\*\\Documents\\*\\".to_owned(),
                    to: "$1/Documents/$2/".to_owned(),
                    filters: vec![no_public_user.clone(),
                    Match {
                        exclude: vec![
                            "My Music".to_owned(),
                            "My Pictures".to_owned(),
                            "My Videos".to_owned(),
                        ],
                        only: vec![],
                    }],
                },
                SrcFile {
                    from: "C:\\Users\\*\\Favorites".to_owned(),
                    to: "$1/Favorites/IE/".to_owned(),
                    filters: vec![no_public_user.clone()],
                },
                SrcFile {
                    from: "C:\\Users\\*\\AppData\\Local\\Packages\\Microsoft.MicrosoftEdge_*\\AC\\MicrosoftEdge\\User\\*\\Favorites".to_owned(),
                    to: "$1/Favorites/Edge/$3".to_owned(),
                    filters: vec![no_public_user.clone()],
                },
                SrcFile {
                    from: "C:\\Users\\*\\AppData\\Local\\Google\\Chrome\\User Data\\*\\Bookmarks".to_owned(),
                    to: "$1/Favorites/Chrome/$2/Bookmarks".to_owned(),
                    filters: vec![no_public_user.clone()],
                },
                SrcFile {
                    from: "C:\\Users\\*\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\*\\*.sqlite".to_owned(),
                    to: "$1/Favorites/Firefox/$2/$3".to_owned(),
                    filters: vec![
                        no_public_user,
                        Match::default(),
                        Match {
                            exclude: vec![],
                            only: vec![
                                "places".to_owned(),
                                "favicons".to_owned(),
                            ]
                        }
                    ],
                },
            ],
        }
    }
}

impl Settings {
    pub fn new(file: Option<&str>) -> Result<Self, Error> {
        let mut s = Config::new();

        s.merge(File::from_str(
            &serde_yaml::to_string(&Settings::default())?,
            FileFormat::Yaml,
        ))?;

        match file {
            Some(file) => {
                s.merge(File::with_name(file))?;
            }
            _ => {}
        }

        s.merge(Environment::with_prefix("ubackup"))?;

        Ok(s.try_into()?)
    }
}
