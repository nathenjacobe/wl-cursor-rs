wanted to learn more about wayland, so I decided to try building this simple tool that prints out the screen location of the cursor on click

you could do this, for example, as a zshrc alias to get the colour of a pixel on click:

```zsh
pixel-colour() {
    local coords x y grim_coords colour

    coords=$(wl-cursor 2>/dev/null | tail -n 1)
    read -r x y <<< "$coords"
    grim_coords="${x},${y} 1x1"

    colour=$(grim -g "$grim_coords" -t ppm - |
        ffmpeg -i pipe:0 \
            -f rawvideo \
            -pix_fmt rgb24 \
            - 2>/dev/null |
        od -An -tu1 |
        tr -s ' ' |
        sed 's/^ //')

    printf 'colour is: [%s] @ [%s %s]\n' "$colour" "$x" "$y"
}
```
