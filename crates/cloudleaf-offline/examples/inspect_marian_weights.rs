use candle_core::{DType, Device};
use candle_nn::VarBuilder;
use candle_transformers::models::marian::{Config, MTModel};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: inspect_marian_weights WEIGHTS")?;
    let builder = VarBuilder::from_pth(path, DType::F32, &Device::Cpu)?;
    let _model = MTModel::new(&Config::opus_mt_en_zh(), builder)?;
    println!("actual PTH and Marian tensor shapes loaded");
    Ok(())
}
