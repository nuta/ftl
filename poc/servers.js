async function main(env) {
    env.houston.tx.on('readable', () => {
        const data = env.houston.tx.read();
        if (data) {
            console.log(data.toString());
        }
    });
}