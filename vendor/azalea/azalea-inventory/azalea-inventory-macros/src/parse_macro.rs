use syn::{
    Ident, LitInt, Token, braced,
    parse::{Parse, ParseStream, Result},
};

pub struct Field {
    pub name: Ident,
    pub length: usize,
}
impl Parse for Field {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<Ident>()?;
        let _ = input.parse::<Token![:]>()?;
        let length = input.parse::<LitInt>()?.base10_parse()?;
        Ok(Self { name, length })
    }
}

pub struct Menu {
    pub name: Ident,
    pub fields: Vec<Field>,
}

impl Parse for Menu {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<Ident>()?;

        let content;
        braced!(content in input);
        let fields = content
            .parse_terminated(Field::parse, Token![,])?
            .into_iter()
            .collect();

        Ok(Self { name, fields })
    }
}

pub struct DeclareMenus {
    pub menus: Vec<Menu>,
}
impl Parse for DeclareMenus {
    fn parse(input: ParseStream) -> Result<Self> {
        let menus = input
            .parse_terminated(Menu::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self { menus })
    }
}
