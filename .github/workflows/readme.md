## Re-useable Pipelines

### reuseable rust build, test, lint and formatting pipeline

```yml
  job1:
    name: CI
    uses: ./.github/workflows/reuseable_ci.yml
    with:
      # rust project location (assumes path to be rust/holo-dns)
      project: holo-dns
```

### digital ocean deployment pipeline

```yml
  # push docker image to digital ocean registry
  job2:
    name: Deploy to Dev
    needs: CI
    uses: ./.github/workflows/reuseable_do_deployment.yml
    if: github.ref == 'refs/heads/main'
    with:
      # github environment to use when deploying
      environment: dev
      # rust project location (assumes path to be rust/holo-dns)
      project: holo-dns
    secrets:
      DIGITAL_OCEAN_TOKEN: ${{ secrets.DIGITAL_OCEAN_TOKEN }}
```

```yml
  # push docker image to digital ocean registry
  # and refresh droplets to the pushed docker image
  job2:
    name: Deploy to Dev
    needs: CI
    uses: ./.github/workflows/reuseable_do_deployment.yml
    if: github.ref == 'refs/heads/main'
    with:
      environment: dev
      project: holo-dns
      # json array of droplets to update
      droplets: '["46.101.64.62"]'
      # port to expose for docker
      port: 53
    secrets:
      DIGITAL_OCEAN_TOKEN: ${{ secrets.DIGITAL_OCEAN_TOKEN }}
      # credentials required for ssh'ing into droplets
      SSH_KEY: ${{ secrets.SSH_KEY }}
      SSH_USERNAME: ${{ secrets.SSH_USERNAME }}
```