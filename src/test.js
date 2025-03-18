export async function fetch_group(archive, group) {
    const response = await fetch("https://archive.openrs2.org/caches/runescape/2064/archives/" + archive + "/groups/" + group + ".dat");
    const data = await response.arrayBuffer();
    return data;
}

// export function fetch_test() {
//     async_test();
// }
