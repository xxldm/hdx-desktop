use serde::Serialize;

#[cfg(all(feature = "flavor-local", feature = "flavor-online"))]
compile_error!("只能启用一个 Desktop flavor feature：flavor-local 或 flavor-online。");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopFlavor {
    Local,
    Online,
    Unspecified,
}

pub fn active_flavor() -> DesktopFlavor {
    #[cfg(feature = "flavor-local")]
    {
        return DesktopFlavor::Local;
    }

    #[cfg(feature = "flavor-online")]
    {
        return DesktopFlavor::Online;
    }

    #[cfg(not(any(feature = "flavor-local", feature = "flavor-online")))]
    {
        DesktopFlavor::Unspecified
    }
}

impl DesktopFlavor {
    pub fn label(self) -> &'static str {
        match self {
            DesktopFlavor::Local => "Local",
            DesktopFlavor::Online => "Online",
            DesktopFlavor::Unspecified => "未指定",
        }
    }

    pub fn product_name(self) -> &'static str {
        match self {
            DesktopFlavor::Local => "HDX Desktop Local",
            DesktopFlavor::Online => "HDX Desktop Online",
            DesktopFlavor::Unspecified => "HDX Desktop",
        }
    }

    pub fn includes_all_in_one(self) -> bool {
        matches!(self, DesktopFlavor::Local)
    }

    pub fn remote_endpoint_required(self) -> bool {
        matches!(self, DesktopFlavor::Online)
    }

    pub fn local_actor(self) -> Option<&'static str> {
        match self {
            DesktopFlavor::Local => Some("LOCAL_ADMIN:local-admin"),
            DesktopFlavor::Online | DesktopFlavor::Unspecified => None,
        }
    }
}
