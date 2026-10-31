use calamine::{open_workbook_auto, Data, Reader};
use rust_xlsxwriter::{Formula, Workbook};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path};

const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_CELLS: usize = 250_000;

fn required_str<'a>(value: &'a Value, label: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{label} inválido na planilha."))
}

fn required_array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{label} inválido na planilha."))
}

fn object_keys(value: &Value, allowed: &[&str], required: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("Parâmetros da planilha inválidos.")?;
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || required.iter().any(|key| !object.contains_key(*key))
    {
        return Err("Os parâmetros da planilha não correspondem ao esquema permitido.".into());
    }
    Ok(())
}

fn validate_formula(formula: &str) -> Result<(), String> {
    if formula.len() > 256
        || !formula.starts_with('=')
        || !formula.is_ascii()
        || formula
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || "=()+-*/^,:. $".contains(ch)))
    {
        return Err("Fórmula inválida; use referências locais e operadores simples.".into());
    }
    let upper = formula.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    for index in 1..bytes.len() {
        if bytes[index] == b'(' {
            let mut start = index;
            while start > 0 && bytes[start - 1].is_ascii_alphabetic() {
                start -= 1;
            }
            let function = &upper[start..index];
            if !["SUM", "AVERAGE", "MIN", "MAX", "COUNT", "IF"].contains(&function) {
                return Err("A fórmula contém uma função não permitida.".into());
            }
        }
    }
    if formula.contains("..") || formula.contains("::") || formula.contains("$") {
        return Err("Fórmulas com referências externas ou absolutas não são permitidas.".into());
    }
    Ok(())
}

pub(crate) fn validate_create(params: &Value) -> Result<(), String> {
    if crate::memory_skills::contains_sensitive_data(&params["sheets"].to_string()) {
        return Err("A planilha contém dado confidencial e não pode ser gravada.".into());
    }
    object_keys(params, &["path", "sheets"], &["path", "sheets"])?;
    let path = params["path"].as_str().ok_or("Informe o caminho .xlsx.")?;
    if path.len() > 4096
        || !Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("xlsx"))
    {
        return Err("O arquivo precisa ter extensão .xlsx.".into());
    }
    if Path::new(path).is_absolute() {
        crate::security::skills::validate_raw_folder_path(path)?;
    } else {
        crate::security::workspace::relative_path(path)?;
    }
    let sheets = params["sheets"]
        .as_array()
        .filter(|sheets| !sheets.is_empty() && sheets.len() <= 10)
        .ok_or("Informe de 1 a 10 abas.")?;
    let mut names = std::collections::HashSet::new();
    let mut cells = 0usize;
    let mut text_bytes = 0usize;
    for sheet in sheets {
        object_keys(
            sheet,
            &["name", "headers", "rows"],
            &["name", "headers", "rows"],
        )?;
        let name = sheet["name"].as_str().ok_or("Nome de aba inválido.")?;
        if name.trim().is_empty()
            || name.chars().count() > 31
            || name.chars().any(|ch| "[]:*?/\\".contains(ch))
            || name.starts_with('\'')
            || name.ends_with('\'')
            || !names.insert(name.to_lowercase())
        {
            return Err("Nome de aba inválido ou duplicado.".into());
        }
        let headers = sheet["headers"]
            .as_array()
            .filter(|headers| !headers.is_empty() && headers.len() <= 100)
            .ok_or("Cada aba precisa de 1 a 100 cabeçalhos.")?;
        for header in headers {
            let value = header
                .as_str()
                .filter(|value| value.len() <= 256)
                .ok_or("Cabeçalho inválido.")?;
            text_bytes = text_bytes.saturating_add(value.len());
        }
        let rows = sheet["rows"]
            .as_array()
            .filter(|rows| rows.len() <= 10_000)
            .ok_or("Cada aba aceita até 10.000 linhas.")?;
        cells = cells.saturating_add(headers.len()).saturating_add(
            rows.iter()
                .map(|row| row.as_array().map_or(0, Vec::len))
                .sum::<usize>(),
        );
        for row in rows {
            let row = row
                .as_array()
                .filter(|row| row.len() <= headers.len())
                .ok_or("Uma linha excede o número de cabeçalhos.")?;
            for cell in row {
                match cell {
                    Value::String(value) if value.len() <= 4096 => {
                        text_bytes = text_bytes.saturating_add(value.len());
                    }
                    Value::Number(number) if number.as_f64().is_some_and(f64::is_finite) => {}
                    Value::Bool(_) => {}
                    Value::Object(_) => {
                        object_keys(cell, &["formula"], &["formula"])?;
                        let formula = cell["formula"].as_str().ok_or("Fórmula inválida.")?;
                        validate_formula(formula)?;
                        text_bytes = text_bytes.saturating_add(formula.len());
                    }
                    _ => return Err("Valor de célula inválido ou acima do limite.".into()),
                }
            }
        }
    }
    if cells > MAX_CELLS {
        return Err("A planilha excede o limite de 250.000 células.".into());
    }
    if text_bytes > MAX_INPUT_BYTES as usize {
        return Err("O conteúdo da planilha excede 16 MB.".into());
    }
    Ok(())
}

pub(crate) fn create_xlsx(root: &Path, params: &Value) -> Result<String, String> {
    validate_create(params)?;
    let raw = required_str(&params["path"], "Caminho")?;
    let path = crate::security::workspace::create_safe_target(root, raw)?;
    if path.exists() {
        return Err("O arquivo de destino já existe; a criação foi recusada.".into());
    }
    let mut workbook = Workbook::new();
    for sheet_data in required_array(&params["sheets"], "Lista de abas")? {
        let worksheet = workbook
            .add_worksheet()
            .set_name(required_str(&sheet_data["name"], "Nome da aba")?)
            .map_err(|_| "Não foi possível preparar o nome da aba XLSX.".to_string())?;
        for (column, header) in
            required_array(&sheet_data["headers"], "Cabeçalhos")?.iter().enumerate()
        {
            worksheet
                .write(column_to_row(0), column as u16, required_str(header, "Cabeçalho")?)
                .map_err(|_| "Não foi possível escrever cabeçalho XLSX.".to_string())?;
        }
        for (row_index, row) in required_array(&sheet_data["rows"], "Linhas")?.iter().enumerate() {
            for (column, cell) in required_array(row, "Linha")?.iter().enumerate() {
                let row = column_to_row(row_index + 1);
                let column = column as u16;
                let result = match cell {
                    Value::String(value) => worksheet.write_string(row, column, value),
                    Value::Number(value) => {
                        worksheet.write_number(
                            row,
                            column,
                            value.as_f64().ok_or("Número de célula inválido na planilha.")?,
                        )
                    }
                    Value::Bool(value) => worksheet.write_boolean(row, column, *value),
                    Value::Object(_) => worksheet.write_formula(
                        row,
                        column,
                        Formula::new(required_str(&cell["formula"], "Fórmula")?),
                    ),
                    _ => return Err("Tipo de célula inválido na planilha.".into()),
                };
                result.map_err(|_| "Não foi possível escrever célula XLSX.".to_string())?;
            }
        }
    }
    let bytes = workbook
        .save_to_buffer()
        .map_err(|_| "Não foi possível gerar o arquivo XLSX.".to_string())?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err("O XLSX excede o limite de 32 MB.".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "Não foi possível criar o XLSX; o arquivo pode já existir.".to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_data())
        .map_err(|_| "Não foi possível gravar o XLSX.".to_string())?;
    Ok(format!("Planilha XLSX criada: {}", path.display()))
}

fn column_to_row(index: usize) -> u32 {
    index as u32
}

fn parse_csv(bytes: &[u8], max_rows: usize) -> Result<Vec<Vec<String>>, String> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| "O CSV não está em UTF-8.".to_string())?;
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    cell.push('"');
                }
                '"' => quoted = false,
                _ => cell.push(ch),
            }
        } else {
            match ch {
                '"' if cell.is_empty() => quoted = true,
                ',' => row.push(std::mem::take(&mut cell)),
                '\n' | '\r' => {
                    if ch == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    row.push(std::mem::take(&mut cell));
                    rows.push(std::mem::take(&mut row));
                    if rows.len() > max_rows {
                        return Err("O CSV excede o limite de linhas solicitado.".into());
                    }
                }
                _ => cell.push(ch),
            }
        }
    }
    if quoted {
        return Err("O CSV contém aspas sem fechamento.".into());
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    if rows.len() > max_rows {
        return Err("O CSV excede o limite de linhas solicitado.".into());
    }
    Ok(rows)
}

pub(crate) fn read(root: &Path, params: &Value) -> Result<String, String> {
    object_keys(params, &["path", "max_rows"], &["path", "max_rows"])?;
    let raw = params["path"]
        .as_str()
        .ok_or("Informe o arquivo da planilha.")?;
    let max_rows = params["max_rows"]
        .as_u64()
        .filter(|value| (1..=1000).contains(value))
        .ok_or("O limite deve ficar entre 1 e 1.000 linhas.")? as usize;
    let path = crate::security::workspace::safe_target(root, raw)?;
    let metadata =
        fs::metadata(&path).map_err(|_| "Não foi possível localizar a planilha.".to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_INPUT_BYTES {
        return Err("A planilha deve ser um arquivo de até 16 MB.".into());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let output = match extension.as_str() {
        "csv" => {
            json!({"rows": parse_csv(&fs::read(&path).map_err(|_| "Não foi possível ler o CSV.".to_string())?, max_rows)?})
        }
        "xlsx" => {
            let mut workbook = open_workbook_auto(&path)
                .map_err(|_| "O arquivo XLSX é inválido ou não pode ser lido.".to_string())?;
            let names = workbook.sheet_names().to_vec();
            let mut sheets = Vec::new();
            for name in names {
                let range = workbook
                    .worksheet_range(&name)
                    .map_err(|_| "Não foi possível ler uma aba do XLSX.".to_string())?;
                let mut rows = Vec::new();
                for row in range.rows().take(max_rows + 1) {
                    rows.push(row.iter().map(Data::to_string).collect::<Vec<_>>());
                }
                if rows.len() > max_rows {
                    return Err("O XLSX excede o limite de linhas solicitado em uma aba.".into());
                }
                sheets.push(json!({"name":name,"rows":rows}));
            }
            json!({"sheets":sheets})
        }
        _ => return Err("A leitura aceita somente arquivos .csv ou .xlsx.".into()),
    };
    let output = serde_json::to_string(&output)
        .map_err(|_| "Não foi possível preparar a planilha lida.".to_string())?;
    if crate::memory_skills::contains_sensitive_data(&output) {
        return Err(
            "A leitura foi interrompida porque o conteúdo contém dado confidencial.".into(),
        );
    }
    if output.len() > 1_000_000 {
        return Err("O resultado da leitura excede 1 MB.".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "odekrai-sheet-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path.canonicalize().unwrap()
    }

    #[test]
    fn creates_valid_multisheet_xlsx_with_values_and_formula_and_reads_limited_rows() {
        let root = temp_root();
        let params = json!({"path":"mock.xlsx","sheets":[{"name":"Dados","headers":["valor","total"],"rows":[[2,{"formula":"=SUM(A2:A2)"}]]},{"name":"Resumo","headers":["nome"],"rows":[["teste"]]}]});
        create_xlsx(&root, &params).unwrap();
        let mut workbook = open_workbook_auto(root.join("mock.xlsx")).unwrap();
        let range = workbook.worksheet_range("Dados").unwrap();
        assert_eq!(range.get_value((1, 0)), Some(&Data::Float(2.0)));
        assert_eq!(
            workbook
                .worksheet_formula("Dados")
                .unwrap()
                .get_value((1, 1))
                .map(String::as_str),
            Some("SUM(A2:A2)")
        );
        fs::write(root.join("table.csv"), "a,b\n\"x,y\",2\n").unwrap();
        assert!(read(&root, &json!({"path":"table.csv","max_rows":1})).is_err());
        assert!(read(&root, &json!({"path":"table.csv","max_rows":2}))
            .unwrap()
            .contains("x,y"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_tabs_external_formulas_and_existing_output() {
        let root = temp_root();
        let absolute = json!({"path":std::env::temp_dir().join("allowed.xlsx").to_string_lossy(),"sheets":[{"name":"Dados","headers":["x"],"rows":[]}]});
        assert!(validate_create(&absolute).is_ok());
        let sensitive = json!({"path":"private.xlsx","sheets":[{"name":"Dados","headers":["documento"],"rows":[["CPF 529.982.247-25"]]}]});
        assert!(validate_create(&sensitive).is_err());
        let duplicate = json!({"path":"x.xlsx","sheets":[{"name":"Dados","headers":["x"],"rows":[]},{"name":"dados","headers":["x"],"rows":[]}]});
        assert!(validate_create(&duplicate).is_err());
        let external_formula = json!({"path":"x.xlsx","sheets":[{"name":"Dados","headers":["x"],"rows":[[{"formula":"=HYPERLINK(\"https://evil\")"}]]}]});
        assert!(validate_create(&external_formula).is_err());
        fs::write(root.join("existing.xlsx"), b"keep").unwrap();
        let existing =
            json!({"path":"existing.xlsx","sheets":[{"name":"Dados","headers":["x"],"rows":[]}]});
        assert!(create_xlsx(&root, &existing).is_err());
        assert_eq!(fs::read(root.join("existing.xlsx")).unwrap(), b"keep");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_xlsx_input_returns_a_validation_error() {
        let root = temp_root();
        let error = create_xlsx(&root, &json!({})).unwrap_err();
        assert!(error.contains("Parâmetros da planilha inválidos"));
        let _ = fs::remove_dir_all(root);
    }
}
