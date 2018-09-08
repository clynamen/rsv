#[macro_use]
extern crate vulkano;
#[macro_use]
extern crate vulkano_shader_derive;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

pub mod shaders;
pub use shaders::*;