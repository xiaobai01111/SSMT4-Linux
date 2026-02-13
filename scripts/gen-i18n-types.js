import fs from 'fs'

const en = JSON.parse(fs.readFileSync('internationalization/en.json'))

function flatten(obj, prefix = '') {
    let res = []
    for (const k in obj) {
        const key = prefix ? `${prefix}.${k}` : k
        if (typeof obj[k] === 'object') res.push(...flatten(obj[k], key))
        else res.push(key)
    }
    return res
}

const keys = flatten(en)

const ts = `
export type I18nKey =
${keys.map(k => `  | '${k}'`).join('\n')}
`

fs.writeFileSync('src/i18n/types.d.ts', ts)