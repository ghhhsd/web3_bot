use jito_sdk_rust::JitoJsonRpcSDK;
use crate::utils::jjj::import_env_var;

fn get_jito_sdk(uuid_string:Option<String>){
    // "https://mainnet.block-engine.jito.wtf/api/v1"
    let base_api_url =import_env_var("JITO_BLOCK_ENGINE_URL");
    JitoJsonRpcSDK::new(base_api_url+"/api/v1", uuid_string);
}
pub async fn get_tip_account(uuid_string:Option<String>){
    let jito_sdk = get_jito_sdk(uuid_string);
    
}
pub fn get_tip_value(){

}

pub fn wait_for_bundle_confirmation(){

}