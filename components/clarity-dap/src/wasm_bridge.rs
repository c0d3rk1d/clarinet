use crate::dap::DAPDebugger;
use chainhook_types::StacksNetwork;
use clarinet_deployments::{
    generate_default_deployment, initiate_session_from_deployment, setup_session_with_deployment,
    update_session_with_contracts_executions,
};
use clarinet_files::{FileAccessor, FileLocation, ProjectManifest, WASMFileSystemAccessor};
use clarity_repl::clarity::analysis::AnalysisDatabase;
use clarity_repl::clarity::ast::build_ast_with_rules;
use clarity_repl::clarity::consts::CHAIN_ID_TESTNET;
use clarity_repl::clarity::contexts::GlobalContext;
use clarity_repl::clarity::costs::LimitedCostTracker;
use clarity_repl::clarity::database::ClarityDatabase;
use clarity_repl::clarity::stacks_common::types::StacksEpochId;
use clarity_repl::clarity::vm::eval;
use clarity_repl::clarity::vm::types::{
    PrincipalData, QualifiedContractIdentifier, StandardPrincipalData,
};
use clarity_repl::clarity::{
    CallStack, ClarityVersion, ContractContext, ContractName, Environment, LocalContext,
    SymbolicExpression, SymbolicExpressionType,
};
use clarity_repl::repl::{ClarityCodeSource, ClarityContract, ContractDeployer};
use debug_types::requests;
use js_sys::{Function as JsFunction, Int32Array};
use serde_wasm_bindgen::from_value as decode_from_js;
use std::panic;
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
    file_accessor: JsFunction,
}

#[wasm_bindgen]
impl DapWasmBridge {
    #[wasm_bindgen(constructor)]
    pub fn new(
        file_accessor: JsFunction,
        send_response: JsFunction,
        send_event: JsFunction,
    ) -> Self {
        panic::set_hook(Box::new(console_error_panic_hook::hook));

        Self {
            dap: DAPDebugger::new(send_response, send_event),
            file_accessor,
        }
    }

    #[wasm_bindgen(js_name = "handleMessage")]
    pub fn handle_message(
        &mut self,
        seq: i64,
        request: String,
        js_params: JsValue,
        wp: Int32Array,
    ) -> Result<bool, String> {
        use requests::*;

        log!(">rust- request: {:?}", &request);
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
                Ok(self.dap.launch(seq, arguments, wp))
            }
            "configuration_done" => Ok(self.dap.configuration_done(seq)),
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
            _ => Err("unsupported request".to_string()),
        }

        // let serializer = Serializer::json_compatible();
        // log!("> repsonse: {:?}", &response);
        // let _ = match response.serialize(&serializer).map_err(|_| JsValue::NULL) {
        //     Ok(r) => self.send_response.call1(&JsValue::NULL, &r),
        //     Err(err) => self.send_response.call1(&JsValue::NULL, &err),
        // };

        // // self.send_response.call1(&JsValue::NULL, &js_response);
    }

    #[wasm_bindgen(js_name = "runDap")]
    pub async fn run_dap(&mut self) -> Result<(), String> {
        match &self.dap.launched {
            Some((manifest_location_str, expression)) => {
                let file_accessor: Box<dyn FileAccessor> =
                    Box::new(WASMFileSystemAccessor::new(self.file_accessor.clone()));

                let manifest_location = FileLocation::from_url_string(&manifest_location_str)?;

                let manifest =
                    ProjectManifest::from_file_accessor(&manifest_location, &file_accessor).await?;

                let (deployment, artifacts) = generate_default_deployment(
                    &manifest,
                    &StacksNetwork::Simnet,
                    false,
                    Some(&file_accessor),
                    Some(StacksEpochId::Epoch21),
                )
                .await?;

                let mut session =
                    setup_session_with_deployment(&manifest, &deployment, Some(&artifacts.asts))
                        .session;

                for (contract_id, (_, location)) in deployment.contracts.iter() {
                    self.dap
                        .path_to_contract_id
                        .insert(location.clone(), contract_id.clone());
                    self.dap
                        .contract_id_to_path
                        .insert(contract_id.clone(), location.clone());
                }

                // let mut ds = session.interpreter.datastore.clone();
                // let mut analysis_db = AnalysisDatabase::new(&mut ds);

                // let conn = ClarityDatabase::new(
                //     &mut ds,
                //     &session.interpreter.burn_datastore,
                //     &session.interpreter.burn_datastore,
                // );

                // let mut global_context = GlobalContext::new(
                //     false,
                //     CHAIN_ID_TESTNET,
                //     conn,
                //     LimitedCostTracker::new_free(),
                //     StacksEpochId::Epoch21,
                // );

                // let context = LocalContext::new();
                // let mut call_stack = CallStack::new();
                // log!("> expression: {:?}", &expression);

                // let contract = ClarityContract {
                //     code_source: ClarityCodeSource::ContractInMemory(expression.to_string()),
                //     name: format!("contract-{}", session.contracts.len()),
                //     deployer: ContractDeployer::DefaultDeployer,
                //     clarity_version: ClarityVersion::default_for_epoch(session.current_epoch),
                //     epoch: session.current_epoch,
                // };
                // log!("> contract.deployer: {:?}", &contract.deployer);

                // let contract_identifier = contract.expect_resolved_contract_identifier(Some(
                //     &session.interpreter.get_tx_sender(),
                // ));

                // log!("> contract_identifier: {:?}", &contract_identifier);

                // let mut contract_context =
                //     ContractContext::new(contract_identifier.clone(), ClarityVersion::Clarity2);
                // let (mut ast, mut diagnostics, success) = session.interpreter.build_ast(&contract);
                // let _ = global_context.execute(|g| {
                //     let mut env = Environment::new(
                //         g,
                //         &mut contract_context,
                //         &mut call_stack,
                //         None,
                //         None,
                //         None,
                //     );
                //     // let result = match ast.expressions[0].expr {
                //     //     SymbolicExpressionType::List(ref expression) => match expression[0].expr {
                //     //         SymbolicExpressionType::Atom(ref name)
                //     //             if name.to_string() == "contract-call?" =>
                //     //         {
                //     //             log!("> expression: {:?}", &expression);
                //     //             let contract_identifier = match expression[1]
                //     //                 .match_literal_value()
                //     //                 .unwrap()
                //     //                 .clone()
                //     //                 .expect_principal()
                //     //             {
                //     //                 PrincipalData::Contract(contract_identifier) => {
                //     //                     contract_identifier
                //     //                 }
                //     //                 _ => unreachable!(),
                //     //             };
                //     //             let method = expression[2].match_atom().unwrap().to_string();
                //     //             let mut args = vec![];
                //     //             for arg in expression[3..].iter() {
                //     //                 let evaluated_arg = eval(arg, &mut env, &context)?;
                //     //                 args.push(SymbolicExpression::atom_value(evaluated_arg));
                //     //             }
                //     //             log!("> contract_identifier: {:?}", &contract_identifier);
                //     //             let contract = g.database.get_contract(&contract_identifier)?;
                //     //             let func = contract.contract_context.lookup_function(&method);
                //     //             log!("> func: {:?}", &func);
                //     //             // eval(exp, env, context)
                //     //         }
                //     //         _ => panic!(),
                //     //     },
                //     //     _ => panic!(),
                //     // };

                //     log!("> result: {:?}", &result);
                // });

                // Begin execution of the expression in debug mode
                match session.eval(expression.clone(), Some(vec![&mut self.dap]), false) {
                    Ok(_result) => Ok(()),
                    Err(_diagnostics) => Err("unable to interpret expression".to_string()),
                }
                // Ok(())
            }
            None => Err("failed to launch".to_string()),
        }
    }
}

fn get_ast(source: &str) -> Vec<SymbolicExpression> {
    let contract_ast = build_ast_with_rules(
        &QualifiedContractIdentifier::transient(),
        source,
        &mut (),
        ClarityVersion::Clarity1,
        StacksEpochId::Epoch21,
        clarity_repl::clarity::ast::ASTRules::Typical,
    )
    .unwrap();
    return contract_ast.expressions;
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
