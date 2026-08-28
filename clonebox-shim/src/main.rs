use std::env::args;
use clonebox_core::{
    event::Event,
    container::create,
    state::ContainerState,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if args().len() != 4 {
        return Err("shim args number".into());
    }

    let mut args = args();

    args.next();
    
    let container_id = args.next().ok_or("missing container id")?;
    let config_path = args.next().ok_or("missing config path")?;
    let event_socket = args.next().ok_or("missing event socket")?;

    match create(&container_id, &config_path, Some(&event_socket)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let event = Event::new(
                container_id,
                ContainerState::Stopped,
                Some(-1),
            );
            event.send_event(&event_socket).unwrap();
            Err(e.into())
        },
    }
}
