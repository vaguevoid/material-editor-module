use game_asset::ecs_module::GpuInterface;
//use game_asset::resource_managers::gpu_interface::PendingTexture;
use game_asset::resource_managers::texture_asset_manager::PendingTexture;
use void_public::event::graphics::NewTexture;
use void_public::graphics::TextureId;
use void_public::EventWriter;

pub fn register_texture(
    texture_path: &str,
    load_into_atlas: bool,
    gpu_interface: &mut GpuInterface,
    new_texture_event_writer: &EventWriter<NewTexture>,
) -> TextureId {
    let id = gpu_interface
        .texture_asset_manager
        .register_next_texture_id();
    let pending_texture = PendingTexture::new(id, &texture_path.into(), load_into_atlas);
    gpu_interface
        .texture_asset_manager
        .load_texture_by_pending_texture(&pending_texture, new_texture_event_writer)
        .unwrap();
    id
}
