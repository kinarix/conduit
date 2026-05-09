use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    ProcessDeploy,
    ProcessDisable,
    InstanceStart,
    InstanceCancel,
    InstanceRead,
    TaskComplete,
    DecisionDeploy,
    SecretManage,
    UserManage,
    RoleManage,
    WorkerManage,
    OrgManage,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProcessDeploy => "process.deploy",
            Self::ProcessDisable => "process.disable",
            Self::InstanceStart => "instance.start",
            Self::InstanceCancel => "instance.cancel",
            Self::InstanceRead => "instance.read",
            Self::TaskComplete => "task.complete",
            Self::DecisionDeploy => "decision.deploy",
            Self::SecretManage => "secret.manage",
            Self::UserManage => "user.manage",
            Self::RoleManage => "role.manage",
            Self::WorkerManage => "worker.manage",
            Self::OrgManage => "org.manage",
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Permission {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "process.deploy" => Ok(Self::ProcessDeploy),
            "process.disable" => Ok(Self::ProcessDisable),
            "instance.start" => Ok(Self::InstanceStart),
            "instance.cancel" => Ok(Self::InstanceCancel),
            "instance.read" => Ok(Self::InstanceRead),
            "task.complete" => Ok(Self::TaskComplete),
            "decision.deploy" => Ok(Self::DecisionDeploy),
            "secret.manage" => Ok(Self::SecretManage),
            "user.manage" => Ok(Self::UserManage),
            "role.manage" => Ok(Self::RoleManage),
            "worker.manage" => Ok(Self::WorkerManage),
            "org.manage" => Ok(Self::OrgManage),
            _ => Err(()),
        }
    }
}
