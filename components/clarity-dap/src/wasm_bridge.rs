use crate::dap::DAPDebugger;
use chainhook_types::StacksNetwork;
use clarinet_deployments::{generate_default_deployment, setup_session_with_deployment};
use clarinet_files::{FileAccessor, FileLocation, ProjectManifest, WASMFileSystemAccessor};
use clarity_repl::clarity::stacks_common::types::StacksEpochId;
use debug_types::requests;
use js_sys::Function as JsFunction;
use serde_wasm_bindgen::from_value as decode_from_js;
use wasm_bindgen::prelude::*;

#[allow(unused_macros)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        #[cfg(feature = "wasm")]
        web_sys::console::log_1(&format!( $( $t )* ).into());
        #[cfg(not(feature = "wasm"))]
        println!( $($t )*);
    }
}

#[wasm_bindgen]
pub struct DapWasmBridge {
    dap: DAPDebugger,
    file_accessor: JsFunction, // send_response: JsFunction,
}

#[wasm_bindgen]
impl DapWasmBridge {
    #[wasm_bindgen(constructor)]
    pub fn new(
        file_accessor: JsFunction,
        send_response: JsFunction,
        send_event: JsFunction,
    ) -> Self {
        Self {
            dap: DAPDebugger::new(send_response, send_event),
            file_accessor,
        }
    }

    #[wasm_bindgen(js_name = "handleMessage")]
    pub async fn handle_message(
        &mut self,
        seq: i64,
        request: String,
        js_params: JsValue,
    ) -> Result<bool, String> {
        log!("> request: {:?}", &request);
        use requests::*;

        match request.as_str() {
            "initialize" => {
                let arguments: InitializeRequestArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.initialize(seq, arguments))
            }
            "launch" => {
                let arguments: LaunchRequestArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                let res = self.dap.launch(seq, arguments);
                match &self.dap.launched {
                    Some((manifest_location_str, expression)) => {
                        let file_accessor: Box<dyn FileAccessor> =
                            Box::new(WASMFileSystemAccessor::new(self.file_accessor.clone()));

                        let manifest_location =
                            FileLocation::from_url_string(&manifest_location_str)?;

                        let project_manifest =
                            ProjectManifest::from_file_accessor(&manifest_location, &file_accessor)
                                .await?;

                        let (deployment, artifacts) = generate_default_deployment(
                            &project_manifest,
                            &StacksNetwork::Simnet,
                            false,
                            Some(&file_accessor),
                            Some(StacksEpochId::Epoch21),
                        )
                        .await?;
                        let mut session = setup_session_with_deployment(
                            &project_manifest,
                            &deployment,
                            Some(&artifacts.asts),
                        )
                        .session;

                        for (contract_id, (_, location)) in deployment.contracts.iter() {
                            self.dap
                                .path_to_contract_id
                                .insert(location.clone(), contract_id.clone());
                            self.dap
                                .contract_id_to_path
                                .insert(contract_id.clone(), location.clone());
                        }

                        // Begin execution of the expression in debug mode
                        match session.eval(expression.clone(), Some(vec![&mut self.dap]), false) {
                            Ok(_result) => Ok(res),
                            Err(_diagnostics) => Err("unable to interpret expression".to_string()),
                        }
                    }
                    None => Err("failed to launch".to_string()),
                }
            }
            "configuration_done" => Ok(self.dap.configuration_done(seq)),
            // ConfigurationDone => self.configuration_done(seq),
            "setBreakpoints" => {
                let arguments: SetBreakpointsArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.set_breakpoints(seq, arguments))
            }
            "setExceptionBreakpoints" => {
                let arguments: SetExceptionBreakpointsArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.set_exception_breakpoints(seq, arguments))
            }
            "disconnect" => {
                let arguments: DisconnectArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.disconnect(seq, arguments))
            }
            "threads" => Ok(self.dap.threads(seq)),
            "stackTrace" => {
                let arguments: StackTraceArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.stack_trace(seq, arguments))
            }
            "scopes" => {
                let arguments: ScopesArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.scopes(seq, arguments))
            }
            "variables" => {
                let arguments: VariablesArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.variables(seq, arguments))
            }
            "stepIn" => {
                let arguments: StepInArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.step_in(seq, arguments))
            }
            "stepOut" => {
                let arguments: StepOutArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.step_out(seq, arguments))
            }
            "next" => {
                let arguments: NextArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.next(seq, arguments))
            }
            "continue" => {
                let arguments: ContinueArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.continue_(seq, arguments))
            }
            "pause" => {
                let arguments: PauseArguments = match decode_from_js(js_params) {
                    Ok(params) => params,
                    Err(_err) => panic!(),
                };
                Ok(self.dap.pause(seq, arguments))
            }
            // "evaluate" => {
            //     let arguments: EvaluateArguments = match decode_from_js(js_params) {
            //         Ok(params) => params,
            //         Err(_err) => panic!(),
            //     };
            //     Ok(self.dap.evaluate(seq, arguments, None, None))
            // }
            _ => {
                log!("unsupported request: {}", request);
                Err("unsupported request".to_string())
            }
        }

        // let serializer = Serializer::json_compatible();
        // log!("> repsonse: {:?}", &response);
        // let _ = match response.serialize(&serializer).map_err(|_| JsValue::NULL) {
        //     Ok(r) => self.send_response.call1(&JsValue::NULL, &r),
        //     Err(err) => self.send_response.call1(&JsValue::NULL, &err),
        // };

        // // self.send_response.call1(&JsValue::NULL, &js_response);
    }
}

// pub async fn run_dap() -> Result<(), String> {
//     let mut dap = DAPDebugger::new();
//     match dap.init() {
//         Ok((manifest_location_str, expression)) => {
//             let manifest_location = FileLocation::from_path_string(&manifest_location_str)?;
//             let project_manifest = ProjectManifest::from_location(&manifest_location)?;
//             let (deployment, artifacts) = generate_default_deployment(
//                 &project_manifest,
//                 &StacksNetwork::Simnet,
//                 false,
//                 None,
//                 None,
//             )
//             .await?;
//             let mut session = setup_session_with_deployment(
//                 &project_manifest,
//                 &deployment,
//                 Some(&artifacts.asts),
//             )
//             .session;

//             for (contract_id, (_, location)) in deployment.contracts.iter() {
//                 dap.path_to_contract_id
//                     .insert(PathBuf::from(location.to_string()), contract_id.clone());
//                 dap.contract_id_to_path
//                     .insert(contract_id.clone(), PathBuf::from(location.to_string()));
//             }

//             // Begin execution of the expression in debug mode
//             match session.eval(expression.clone(), Some(vec![&mut dap]), false) {
//                 Ok(_result) => Ok(()),
//                 Err(_diagnostics) => Err("unable to interpret expression".to_string()),
//             }
//         }
//         Err(e) => Err(format!("dap_init: {}", e)),
//     }
// }
