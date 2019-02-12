use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    io::Read,
    mem,
    os::unix::io::AsRawFd,
    rc::Rc,
    thread,
    time::Duration,
};

use os_pipe::pipe;
use wayland_protocols::wlr::unstable::data_control::v1::server::{
    zwlr_data_control_device_v1::{
        Request as ServerDeviceRequest, ZwlrDataControlDeviceV1 as ServerDevice,
    },
    zwlr_data_control_manager_v1::{
        RequestHandler as ServerManagerRequestHandler, ZwlrDataControlManagerV1 as ServerManager,
    },
    zwlr_data_control_source_v1::{
        Request as ServerSourceRequest, ZwlrDataControlSourceV1 as ServerSource,
    },
};
use wayland_server::{protocol::wl_seat::WlSeat as ServerSeat, NewResource};

use crate::{copy::*, tests::TestServer, ClipboardType};

#[test]
fn clear_test() {
    struct ServerManagerHandler {
        pass: Rc<Cell<bool>>,
    }

    impl ServerManagerRequestHandler for ServerManagerHandler {
        fn get_data_device(&mut self,
                           _manager: ServerManager,
                           id: NewResource<ServerDevice>,
                           _seat: ServerSeat) {
            let pass = self.pass.clone();
            id.implement_closure(move |request, _| {
                                     if let ServerDeviceRequest::SetSelection { source: None } =
                                         request
                                     {
                                         pass.set(true);
                                     }
                                 },
                                 None::<fn(_)>,
                                 ());
        }
    }

    let mut server = TestServer::new();
    server.display
          .create_global::<ServerSeat, _>(6, |new_res, _| {
              new_res.implement_dummy();
          });

    let pass = Rc::new(Cell::new(false));
    {
        let pass = pass.clone();
        server.display
              .create_global::<ServerManager, _>(1, move |new_res, _| {
                  new_res.implement(ServerManagerHandler { pass: pass.clone() },
                                    None::<fn(_)>,
                                    ());
              });
    }

    let socket_name = mem::replace(&mut server.socket_name, OsString::new());
    let child =
        thread::spawn(move || clear_internal(ClipboardType::Regular, Seat::All, Some(socket_name)));

    thread::sleep(Duration::from_millis(100));
    server.answer();

    thread::sleep(Duration::from_millis(100));
    server.answer();

    thread::sleep(Duration::from_millis(100));
    server.answer();

    child.join().unwrap().unwrap();

    assert!(pass.get());
}

#[test]
fn copy_test() {
    struct ServerManagerHandler {
        selection: Rc<RefCell<Option<ServerSource>>>,
    }

    impl ServerManagerRequestHandler for ServerManagerHandler {
        fn create_data_source(&mut self, _manager: ServerManager, id: NewResource<ServerSource>) {
            id.implement_closure(|request, source| {
                                     if let ServerSourceRequest::Offer { mime_type } = request {
                                         source.as_ref()
                                               .user_data::<RefCell<Vec<_>>>()
                                               .unwrap()
                                               .borrow_mut()
                                               .push(mime_type);
                                     }
                                 },
                                 None::<fn(_)>,
                                 RefCell::new(Vec::<String>::new()));
        }

        fn get_data_device(&mut self,
                           _manager: ServerManager,
                           id: NewResource<ServerDevice>,
                           _seat: ServerSeat) {
            let selection = self.selection.clone();
            id.implement_closure(move |request, _| {
                                     if let ServerDeviceRequest::SetSelection { source } = request {
                                         *selection.borrow_mut() = source;
                                     }
                                 },
                                 None::<fn(_)>,
                                 ());
        }
    }

    let mut server = TestServer::new();
    server.display
          .create_global::<ServerSeat, _>(6, |new_res, _| {
              new_res.implement_dummy();
          });

    let selection = Rc::new(RefCell::new(None));
    {
        let selection = selection.clone();
        server.display
              .create_global::<ServerManager, _>(1, move |new_res, _| {
                  new_res.implement(ServerManagerHandler { selection: selection.clone() },
                                    None::<fn(_)>,
                                    ());
              });
    }

    let socket_name = mem::replace(&mut server.socket_name, OsString::new());
    let child = thread::spawn(move || {
        let mut opts = Options::new();
        opts.foreground(true);
        copy_internal(opts,
                      Source::Bytes(&[1, 3, 3, 7]),
                      MimeType::Specific("test".to_string()),
                      Some(socket_name))
    });

    thread::sleep(Duration::from_millis(100));
    server.answer();

    thread::sleep(Duration::from_millis(100));
    server.answer();

    thread::sleep(Duration::from_millis(100));
    server.answer();

    let mime_types = selection.borrow().as_ref().map(|x| {
                                                    x.as_ref()
                                                     .user_data::<RefCell<Vec<String>>>()
                                                     .unwrap()
                                                     .borrow()
                                                     .clone()
                                                });

    let (mut read, write) = pipe().unwrap();

    if let Some(source) = selection.borrow().as_ref() {
        source.send("test".to_string(), write.as_raw_fd());
        drop(write);
        source.cancelled();
    }

    thread::sleep(Duration::from_millis(100));
    server.answer();

    let mut contents = vec![];
    read.read_to_end(&mut contents).unwrap();

    child.join().unwrap().unwrap();

    assert_eq!(mime_types, Some(vec!["test".to_string()]));
    assert_eq!(contents, [1, 3, 3, 7]);
}
