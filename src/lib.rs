use std::fmt::Debug;
use futures::Stream;
use futures::StreamExt;
use js_sys::{ArrayBuffer, AsyncIterator, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, stream::JsStream};

pub struct GetFileHandleOptions {
    pub create: bool,
}

impl Default for GetFileHandleOptions {
    fn default() -> Self {
        Self {
            create: false,
        }
    }
}

pub struct GetDirectoryHandleOptions {
    pub create: bool,
}

impl Default for GetDirectoryHandleOptions {
    fn default() -> Self {
        Self {
            create: false,
        }
    }
}

pub struct CreateWritableOptions {
    pub keep_existing_data: bool,
}

impl Default for CreateWritableOptions {
    fn default() -> Self {
        Self {
            keep_existing_data: false,
        }
    }
}

pub struct FileSystemRemoveOptions {
    pub recursive: bool,
}

impl Default for FileSystemRemoveOptions {
    fn default() -> Self {
        Self {
            recursive: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DirectoryEntry {
    File(FileHandle),
    Directory(DirectoryHandle),
}

/// Returns the Origin Private File System's root directory.
pub async fn storage_directory() -> std::io::Result<DirectoryHandle> {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::FileSystemDirectoryHandle;

    let window = web_sys::window().ok_or(std::io::Error::new(std::io::ErrorKind::Other, "No window object"))?;
    let navigator = window.navigator();

    let root_directory_handle =
        FileSystemDirectoryHandle::from(map_io_result(JsFuture::from(navigator.storage().get_directory()).await)?);

    Ok(DirectoryHandle::from(root_directory_handle))
}

#[derive(Debug, Clone)]
pub struct DirectoryHandle(web_sys::FileSystemDirectoryHandle);

#[derive(Debug, Clone)]
pub struct FileHandle(web_sys::FileSystemFileHandle);

#[derive(Debug, Clone)]
pub struct WritableFileStream(web_sys::FileSystemWritableFileStream);

#[derive(Debug, Clone)]
pub struct Blob(web_sys::Blob);

#[derive(Debug, Clone)]
pub struct File(web_sys::File);

#[derive(Debug, Clone)]
pub struct FileList(web_sys::FileList);

pub struct FileListIter {
    list: FileList,
    index: usize,
}

impl From<web_sys::FileSystemDirectoryHandle> for DirectoryHandle {
    fn from(handle: web_sys::FileSystemDirectoryHandle) -> Self {
        Self(handle)
    }
}

impl From<web_sys::FileSystemFileHandle> for FileHandle {
    fn from(handle: web_sys::FileSystemFileHandle) -> Self {
        Self(handle)
    }
}

impl From<web_sys::FileSystemWritableFileStream> for WritableFileStream {
    fn from(handle: web_sys::FileSystemWritableFileStream) -> Self {
        Self(handle)
    }
}

impl From<web_sys::Blob> for Blob {
    fn from(handle: web_sys::Blob) -> Self {
        Self(handle)
    }
}

impl From<web_sys::File> for File {
    fn from(handle: web_sys::File) -> Self {
        Self(handle)
    }
}

impl From<web_sys::FileList> for FileList {
    fn from(handle: web_sys::FileList) -> Self {
        Self(handle)
    }
}

impl DirectoryHandle {
    pub async fn get_file_handle(&self, name: &str) -> std::io::Result<FileHandle> {
        self.get_file_handle_with_options(name, &Default::default()).await
    }

    pub async fn get_file_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetFileHandleOptions,
    ) -> std::io::Result<FileHandle> {
        let fs_options = web_sys::FileSystemGetFileOptions::new();
        fs_options.set_create(options.create);
        let file_system_file_handle = web_sys::FileSystemFileHandle::from(
            map_io_result(JsFuture::from(self.0.get_file_handle_with_options(name, &fs_options)).await)?,
        );
        Ok(FileHandle(file_system_file_handle))
    }

    pub async fn get_directory_handle(&self, name: &str) -> std::io::Result<Self> {
        self.get_directory_handle_with_options(name, &Default::default()).await
    }

    pub async fn get_directory_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetDirectoryHandleOptions,
    ) -> std::io::Result<Self> {
        let fs_options = web_sys::FileSystemGetDirectoryOptions::new();
        fs_options.set_create(options.create);
        let file_system_directory_handle = web_sys::FileSystemDirectoryHandle::from(
            map_io_result(JsFuture::from(self.0.get_directory_handle_with_options(name, &fs_options)).await)?,
        );
        Ok(DirectoryHandle(file_system_directory_handle))
    }

    pub async fn remove_entry(&mut self, name: &str) -> std::io::Result<()> {
        map_io_result(JsFuture::from(self.0.remove_entry(name)).await)?;
        Ok(())
    }

    pub async fn remove_entry_with_options(
        &mut self,
        name: &str,
        options: &FileSystemRemoveOptions,
    ) -> std::io::Result<()> {
        let fs_options = web_sys::FileSystemRemoveOptions::new();
        fs_options.set_recursive(options.recursive);
        map_io_result(JsFuture::from(self.0.remove_entry_with_options(name, &fs_options)).await)?;
        Ok(())
    }

    pub async fn entries(
        &self,
    ) -> std::io::Result<impl Stream<Item = std::io::Result<(String, DirectoryEntry)>>>
    {
        let entries_iterator = self.0.entries();
        let async_iterator = AsyncIterator::from(entries_iterator);
        let js_stream: JsStream = JsStream::from(async_iterator);

        let stream = js_stream.map(|item| {
            map_io_result(match item {
                Ok(js_array) => {
                    // entries() returns [key, value] pairs
                    let array = js_sys::Array::from(&js_array);
                    let filename = array
                        .get(0)
                        .as_string()
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Invalid filename"))?;
                    let handle = array.get(1);

                    // Determine if it's a file or directory handle
                    let entry = if handle.has_type::<web_sys::FileSystemFileHandle>() {
                        DirectoryEntry::File(FileHandle(web_sys::FileSystemFileHandle::from(handle)))
                    } else if handle.has_type::<web_sys::FileSystemDirectoryHandle>() {
                        DirectoryEntry::Directory(DirectoryHandle(web_sys::FileSystemDirectoryHandle::from(
                            handle,
                        )))
                    } else {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unknown handle type"));
                    };

                    Ok((filename, entry))
                }
                Err(e) => Err(e),
            })
        });

        Ok(stream)
    }
}

impl FileHandle {
    pub async fn create_writable(&mut self) -> std::io::Result<WritableFileStream> {
        self.create_writable_with_options(&Default::default()).await
    }

    pub async fn create_writable_with_options(
        &mut self,
        options: &crate::CreateWritableOptions,
    ) -> std::io::Result<WritableFileStream> {
        let fs_options = web_sys::FileSystemCreateWritableOptions::new();
        fs_options.set_keep_existing_data(options.keep_existing_data);
        let file_system_writable_file_stream = web_sys::FileSystemWritableFileStream::unchecked_from_js(
            map_io_result(JsFuture::from(self.0.create_writable_with_options(&fs_options)).await)?,
        );
        Ok(WritableFileStream(file_system_writable_file_stream))
    }

    pub async fn read(&self) -> std::io::Result<Vec<u8>> {
        self.get_blob().await?.binary().await
    }

    pub async fn size(&self) -> std::io::Result<usize> {
        let size = self.get_blob().await?.size();
        Ok(size)
    }
}

impl FileHandle {
    pub async fn get_blob(&self) -> std::io::Result<Blob> {
        let file: web_sys::Blob = map_io_result(JsFuture::from(self.0.get_file()).await)?.into();
        Ok(Blob(file))
    }

    pub async fn get_file(&self) -> std::io::Result<File> {
        let file: web_sys::File = map_io_result(JsFuture::from(self.0.get_file()).await)?.into();
        Ok(File(file))
    }
}

impl WritableFileStream {
    pub async fn write(&mut self, data: Vec<u8>) -> std::io::Result<()> {
        // You'd think we could just do
        // ```
        // JsFuture::from(self.0.write_with_u8_array(data.as_mut_slice())?).await?;
        // ```
        // But a safari bug makes this write basically the entire wasm heap to the file.
        // So we have to write as a blob first.

        let uint8_array = js_sys::Uint8Array::from(data.as_slice());
        let array = js_sys::Array::new();
        array.push(&uint8_array);
        let blob = map_io_result(web_sys::Blob::new_with_u8_array_sequence(&array))?;

        map_io_result(JsFuture::from(map_io_result(self.0.write_with_blob(&blob))?).await)?;
        Ok(())
    }

    pub async fn close(&mut self) -> std::io::Result<()> {
        map_io_result(JsFuture::from(self.0.close()).await)?;
        Ok(())
    }

    pub async fn seek(&mut self, offset: usize) -> std::io::Result<()> {
        map_io_result(JsFuture::from(map_io_result(self.0.seek_with_u32(offset as u32))?).await)?;
        Ok(())
    }
}

impl Blob {
    pub fn size(&self) -> usize {
        self.0.size() as usize
    }

    pub async fn binary(&self) -> std::io::Result<Vec<u8>> {
        let buffer = ArrayBuffer::unchecked_from_js(map_io_result(JsFuture::from(self.0.array_buffer()).await)?);
        let uint8_array = Uint8Array::new(&buffer);
        let mut vec = vec![0; self.size()];
        uint8_array.copy_to(&mut vec);
        Ok(vec)
    }

    #[allow(dead_code)]
    pub async fn text(&self) -> std::io::Result<String> {
        map_io_result(JsFuture::from(self.0.text()).await)?
            .as_string()
            .ok_or(std::io::Error::new(std::io::ErrorKind::Other, "Unknown error"))
    }
}

impl File {
    /// Last modified timestamp since the UNIX epoch.
    pub fn last_modified(&self) -> std::time::SystemTime {
        std::time::SystemTime::UNIX_EPOCH.checked_add(
            std::time::Duration::from_millis(unsafe { self.0.last_modified().to_int_unchecked() })
        ).unwrap()
    }

    /// Filename.
    pub fn name(&self) -> String {
        self.0.name()
    }

    /// Returns the inherited `Blob` interface.
    pub fn as_blob(&self) -> Blob {
        Blob::from(self.0.clone().dyn_into::<web_sys::Blob>().unwrap())
    }
}

impl FileList {
    pub fn len(&self) -> usize {
        self.0.length() as usize
    }

    pub fn iter(&self) -> FileListIter {
        self.clone().into_iter()
    }

    pub fn get(&self, index: usize) -> Option<File> {
        self.0.item(index as u32).map(|f| File::from(f))
    }
}

impl std::iter::IntoIterator for FileList {
    type Item = File;
    type IntoIter = FileListIter;

    fn into_iter(self) -> Self::IntoIter {
        FileListIter {
            list: self,
            index: 0,
        }
    }
}

impl std::iter::Iterator for FileListIter {
    type Item = File;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(item) = self.list.0.item(self.index as u32) else {
            return None;
        };
        self.index += 1;
        Some(File::from(item))
    }
}

fn map_io_result<T>(result: Result<T, JsValue>) -> std::io::Result<T> {
    result.map_err(|e| {
        use wasm_bindgen::JsCast;
        let Ok(e) = e.dyn_into::<js_sys::Error>() else {
            return std::io::Error::new(std::io::ErrorKind::Other, "Unknown error");
        };
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string().as_string().unwrap())
    })
}