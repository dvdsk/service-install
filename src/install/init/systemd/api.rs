use std::time::{Duration, Instant};

use systemd_zbus::zbus::{self, Connection};
use systemd_zbus::{ActiveState, ManagerProxy, Mode};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error connecting to systemd service manger on dbus")]
    ConnectToServiceManager(#[source] zbus::Error),
    #[error("Error connecting to system dbus")]
    ConnectToSystemBus(#[source] zbus::Error),
    #[error("Error connecting to user dbus")]
    ConnectToUserBus(zbus::Error),
    #[error("Error starting unit")]
    StartUnit(zbus::Error),
    #[error("Error stopping unit")]
    StopUnit(zbus::Error),
    #[error("Error restarting unit")]
    RestartUnit(zbus::Error),
    #[error("Error reloading service files")]
    Reload(zbus::Error),
    #[error("Error listing units")]
    ListUnits(zbus::Error),
    #[error("Could not enable service")]
    EnablingService(zbus::Error),
    #[error("More then one unit with the given service name")]
    MoreThenOneUnit,
}

macro_rules! on_seperate_tokio_thread {
    ($code:block) => {
        std::thread::scope(|s| {
            s.spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .expect("should be able to spawn tokio runtime")
                    .block_on(async { $code })
            })
            .join()
            .expect("should not panic")
        })
    };
}
pub(crate) use on_seperate_tokio_thread;

pub(crate) async fn enable_service(service: &str, mode: super::Mode) -> Result<(), Error> {
    let connection = get_connection(mode).await?;
    let manager_proxy = ManagerProxy::new(&connection)
        .await
        .map_err(Error::ConnectToServiceManager)?;
    const ENABLE_PERMANENTLY: bool = true;
    manager_proxy
        .enable_unit_files(&[service], ENABLE_PERMANENTLY, true)
        .await
        .map_err(Error::EnablingService)?;
    Ok(())
}

pub(crate) async fn disable_service(service: &str, mode: super::Mode) -> Result<(), Error> {
    let connection = get_connection(mode).await?;
    let manager_proxy = ManagerProxy::new(&connection)
        .await
        .map_err(Error::ConnectToServiceManager)?;
    const ENABLE_PERMANENTLY: bool = true;
    manager_proxy
        .disable_unit_files(&[service], ENABLE_PERMANENTLY)
        .await
        .map_err(Error::EnablingService)?;
    Ok(())
}

pub(crate) async fn start_service(service: &str, mode: super::Mode) -> Result<(), Error> {
    let connection = get_connection(mode).await?;
    let manager_proxy = ManagerProxy::new(&connection)
        .await
        .map_err(Error::ConnectToServiceManager)?;
    manager_proxy
        .start_unit(service, Mode::Replace)
        .await
        .map_err(Error::StartUnit)?;
    Ok(())
}

pub(crate) async fn stop_service(service: &str, mode: super::Mode) -> Result<(), Error> {
    let connection = get_connection(mode).await?;
    let manager_proxy = ManagerProxy::new(&connection)
        .await
        .map_err(Error::ConnectToServiceManager)?;
    manager_proxy
        .stop_unit(service, Mode::Replace)
        .await
        .map_err(Error::StopUnit)?;
    Ok(())
}

pub(crate) async fn reload(mode: super::Mode) -> Result<(), Error> {
    let connection = get_connection(mode).await?;
    let manager_proxy: ManagerProxy<'_> = ManagerProxy::new(&connection)
        .await
        .map_err(Error::ConnectToServiceManager)?;
    manager_proxy.reload().await.map_err(Error::Reload)
}

pub(crate) async fn get_connection(mode: super::Mode) -> Result<Connection, Error> {
    match mode {
        super::Mode::System => Ok(Connection::system()
            .await
            .map_err(Error::ConnectToSystemBus)?),
        super::Mode::User => Ok(Connection::session()
            .await
            .map_err(Error::ConnectToUserBus)?),
    }
}

pub(crate) async fn restart(service: &str, mode: super::Mode) -> Result<(), Error> {
    let connection = get_connection(mode).await?;
    let manager_proxy = ManagerProxy::new(&connection)
        .await
        .map_err(Error::ConnectToServiceManager)?;
    manager_proxy
        .restart_unit(service, Mode::Replace)
        .await
        .map_err(Error::RestartUnit)?;
    Ok(())
}

pub(crate) async fn unit_activity(
    service: &str,
    mode: super::Mode,
) -> Result<Option<ActiveState>, Error> {
    let connection = get_connection(mode).await?;
    let manager_proxy = ManagerProxy::new(&connection)
        .await
        .map_err(Error::ConnectToServiceManager)?;
    let mut units = manager_proxy
        .list_units_by_names(&[service])
        .await
        .map_err(Error::ListUnits)?
        .into_iter()
        .map(|u| u.active);

    let res = units.next();
    if units.next().is_some() {
        Err(Error::MoreThenOneUnit)
    } else {
        Ok(res)
    }
}

pub(crate) async fn is_active(service: &str, mode: super::Mode) -> Result<bool, Error> {
    Ok(unit_activity(service, mode)
        .await?
        .iter()
        .any(|a| matches!(a, ActiveState::Inactive | ActiveState::Failed)))
}

#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("Can not wait for a service that does not exist")]
    ServiceNotFound,
    #[error("Error listing units")]
    ListUnits(#[source] Error),
    #[error("Waited longer then 10 seconds for unit to become active")]
    TimedOut,
    #[error("Unit failed")]
    UnitFailed,
}

pub(crate) async fn wait_for_active(service: &str, mode: super::Mode) -> Result<(), WaitError> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        let unit = unit_activity(service, mode)
            .await
            .map_err(WaitError::ListUnits)?
            .ok_or(WaitError::ServiceNotFound)?;
        if unit == ActiveState::Active {
            return Ok(());
        }
        if unit == ActiveState::Failed {
            return Err(WaitError::UnitFailed);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(WaitError::TimedOut)
}

pub(crate) async fn wait_for_inactive(service: &str, mode: super::Mode) -> Result<(), WaitError> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        let unit = unit_activity(service, mode)
            .await
            .map_err(WaitError::ListUnits)?
            .ok_or(WaitError::ServiceNotFound)?;
        if unit == ActiveState::Inactive {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(WaitError::TimedOut)
}
